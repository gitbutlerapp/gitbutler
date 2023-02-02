VERSION=2.39.1
ARCH=$(shell rustc -Vv | grep host | cut -f2 -d' ')

git-$(ARCH):
	./build.sh --dist "$(shell pwd)/git-$(ARCH)" --version "$(VERSION)"

all: git-$(ARCH)
