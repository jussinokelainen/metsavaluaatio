APP := metsaval
BIN := target/release/$(APP)
VERSION := $(shell printf "r%s.%s" "$$(git rev-list --count HEAD)" "$$(git rev-parse --short HEAD)")

RS_FILES := $(shell find src/ -name '*.rs')

PREFIX ?= /usr
DESTDIR ?=
BINDIR := $(DESTDIR)$(PREFIX)/bin
# DATADIR := $(DESTDIR)$(PREFIX)/share/$(APP)

.PHONY: build install uninstall clean run debug

build: $(BIN)

$(BIN): $(RS_FILES) Cargo.lock Cargo.toml
	@DATADIR=$(DATADIR) VERSION=$(VERSION) cargo build --release

run:
	@cargo run

debug:
	@cargo build

install: $(BIN)
	@printf "\033[36m==> \033[0mInstalling files...\n"
	@mkdir -p $(BINDIR)
	install -m755 $(BIN) $(BINDIR)/$(APP)
	@printf "[\033[32m OK \033[0m] Installation complete\n"

uninstall:
	@printf "\033[36m==> \033[0mRemoving installed files...\n"
	rm -f $(BINDIR)/$(APP)
	@printf "[\033[32m OK \033[0m] Uninstall complete\n"

clean:
	@cargo clean

