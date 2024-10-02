TEST_PKG1:=tmp/repo/fake_pkg1-2024.04.07-2-any.pkg.tar.zst
TEST_PKG2:=tmp/repo/fake_pkg2-2024.04.07-2-any.pkg.tar.zst
TMP_PKGS:=tmp/pkgs


.PHONY: all test

all:
	cargo build
	
test: $(TEST_PKG1) $(TEST_PKG2) $(TMP_PKGS)
	@cargo test

$(TEST_PKG1): 
	mkdir -p tmp/repo && cd resources/tests/fake_pkg1/ && PKGDEST=../../../tmp/repo makepkg

$(TEST_PKG2): 
	mkdir -p tmp/repo && cd resources/tests/fake_pkg2/ && PKGDEST=../../../tmp/repo makepkg

$(TMP_PKGS):
	mkdir -p tmp/pkgs
	cp -rf resources/tests/fake_pkg1 tmp/pkgs
	cp -rf resources/tests/fake_pkg2 tmp/pkgs
