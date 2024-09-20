TEST_PKG:=resources/tests/fake_pkg/testing_fake_pkg-2024.04.07-2-any.pkg.tar.zst

.PHONY: all test

all:
	cargo build

	
test: $(TEST_PKG)
	cargo test

$(TEST_PKG): 
	cd test/fake_pkg && makepkg
