use crossbeam_channel::{unbounded, Receiver};
use log::error;
use pacage::{
    builder,
    conf::{Conf, Package},
    db,
    format::SrcInfo,
    patch::patch,
};

pub fn dl_and_build(
    conf: &Conf,
    pkgbuilds: Receiver<(SrcInfo, Package)>,
    continue_on_e: bool,
) -> Result<usize, String> {
    let (src_to_dl_sender, src_to_dl) = unbounded::<(SrcInfo, Package)>();
    let (source_dl_sender, source_dl) = unbounded::<(SrcInfo, Package)>();
    let dbpkgs = db::list(&conf);
    // Check if package is already there
    // TODO: spawn it own thread
    // TODO: check if pkg is lower
    'pkg: while let Ok((wanted_srcinfo, wanted_pkg)) = pkgbuilds.recv() {
        if let Ok(dbpkgs) = &dbpkgs {
            for db_package in dbpkgs {
                if wanted_srcinfo.name == db_package.name
                    && wanted_srcinfo.get_version() != db_package.get_version()
                {
                    // Already up to date
                    src_to_dl_sender.send((wanted_srcinfo, wanted_pkg));
                    continue 'pkg;
                }
            }
        }
    }
    drop(src_to_dl_sender);
    // TODO: before
    let builder = builder::Builder::new(&conf).map_err(|e| e.to_string())?;

    // TOOD: spawn thread
    builder
        .download_srcs(&conf, src_to_dl, source_dl_sender)
        .unwrap();

    let mut pkgbuilds = Vec::new();
    while let Ok((srcinfo, pkg)) = source_dl.recv() {
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

    db::add(&conf, &pkgbuilds).map_err(|e| e.to_string())?;
    Ok(pkgbuilds.len())
}
