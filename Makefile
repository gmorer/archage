TEST_PKG:=tmp/repo/testing_fake_pkg-2024.04.07-2-any.pkg.tar.zst

.PHONY: all test

all:
	cargo build
	
test: $(TEST_PKG)
	cargo test

$(TEST_PKG): 
	mkdir -p tmp/repo && cd resources/tests/fake_pkg/ && PKGDEST=../../../tmp/repo makepkg
