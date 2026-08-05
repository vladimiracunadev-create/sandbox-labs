.PHONY: check lock test build dashboard doctor clean zip
check:
	node scripts/validate-config.mjs
	node scripts/check-doc-links.mjs
	node scripts/run-negative-tests.mjs
	cd control-center && node scripts/build.mjs && node --test test/*.test.mjs

lock:
	@test -f Cargo.lock || cargo generate-lockfile

test: check lock
	cargo test --workspace --locked

build: lock
	cargo build --workspace --locked
	cd control-center && node scripts/build.mjs

dashboard:
	cd control-center && node scripts/build.mjs && node dist/server.js

doctor:
	bash scripts/doctor.sh

clean:
	rm -rf target .sandbox-data
	find evidence/runs -type f -name '*.json' -delete

zip:
	bash scripts/package-release.sh
