CARGO := cargo

all: build clippy test

build:
	@echo "Building project... " && \
		$(CARGO) build --release -q && \
		echo "done." || echo "Failed."

test:
	@$(CARGO) test -q

clippy:
	@echo "Running clippy... " && \
	$(CARGO) clippy -q --color=always && \
	echo "done." || echo "Failed."


coverage:
	@$(CARGO) tarpaulin --out=Html --output-dir /tmp/cov-output && \
		type -p open && cmd=open || type -p xdg-open && cmd=xdg-open; \
		$$cmd /tmp/cov-output/tarpaulin-report.html

release:
	./scripts/make-release.sh

.PHONY: all build test clippy coverage release
