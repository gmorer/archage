use crossbeam_channel::{unbounded, Receiver};
use log::{error, info};
use pacage::{
    builder::{Builder, BuilderError},
    conf::{Conf, Package},
    db,
    format::{DbDesc, SrcInfo},
    patch::patch,
};

fn is_outdated(dbpkgs: &Vec<DbDesc>, pkg: &SrcInfo) -> bool {
    for dbpkg in dbpkgs {
        if dbpkg.name == pkg.name {
            return dbpkg.get_version() != pkg.get_version();
        }
    }
    false
}

pub fn dl_and_build(
    conf: &Conf,
    pkgbuilds: Receiver<(SrcInfo, Package)>,
    builder_recv: Receiver<Result<Builder, BuilderError>>,
    continue_on_e: bool,
) -> Result<usize, String> {
    let (src_to_dl_sender, src_to_dl) = unbounded::<(SrcInfo, Package)>();
    let (source_dl_sender, source_dl) = unbounded::<(SrcInfo, Package)>();
    let dbpkgs = db::list(&conf);
    // Check if package is already there
    // TODO: spawn it own thread
    // TODO: check if pkg is lower
    while let Ok((wanted_srcinfo, wanted_pkg)) = pkgbuilds.recv() {
        if let Ok(dbpkgs) = &dbpkgs {
            if is_outdated(dbpkgs, &wanted_srcinfo) {
                src_to_dl_sender.send((wanted_srcinfo, wanted_pkg));
            } else {
                info!("[{}] Already up to date", wanted_srcinfo.name);
            }
        } else {
            src_to_dl_sender.send((wanted_srcinfo, wanted_pkg));
        }
    }
    drop(src_to_dl_sender);
    // TODO: before
    let builder = match builder_recv.recv() {
        Ok(Ok(builder)) => builder,
        Err(e) => Err(format!("Failed to recv builder: {}", e))?,
        Ok(Err(e)) => Err(format!("Failed to create builder: {}", e))?,
    };

    // TOOD: spawn thread
    builder
        .download_srcs(&conf, src_to_dl, source_dl_sender)
        .unwrap();

    let mut pkgbuilds = Vec::new();
    while let Ok((srcinfo, pkg)) = source_dl.recv() {
        if let Ok(dbpkgs) = &dbpkgs {
            if !is_outdated(dbpkgs, &srcinfo) {
                info!("[{}] Already up to date", srcinfo.name);
                continue;
            }
        }
        if let Err(e) = patch(&conf, &srcinfo) {
            let e = format!("[{}] Skipping build, failed to patch: {}", srcinfo.name, e);
            if continue_on_e {
                error!("{}", e);
            } else {
                return Err(e);
            }
        } else if let Err(e) = builder.build_pkg(&conf, &pkg) {
            let e = format!("[{}] Skipping build, failed to build: {}", srcinfo.name, e);
            if continue_on_e {
                error!("{}", e);
            } else {
                return Err(e);
            }
        } else {
            pkgbuilds.push(srcinfo);
        }
    }

    if !pkgbuilds.is_empty() {
        db::add(&conf, &pkgbuilds).map_err(|e| e.to_string())?;
    }
    Ok(pkgbuilds.len())
}
