.PHONY: build-node build-browser test-node test-browser

CARGO_FLAGS ?= --dev
build-node:
	wasm-pack build $(CARGO_FLAGS) --target nodejs
	rm pkg/.gitignore pkg/package.json
	cp -r pkg/. ../languages/js/src
	rm -rf pkg

build-browser:
	wasm-pack build $(CARGO_FLAGS) --target bundler

test-node:
	wasm-pack test --node

test-browser:
	# TODO(gj): Firefox once https://github.com/rustwasm/wasm-bindgen/issues/2261
	# is fixed.
	#
	# TODO(gj): Safari works locally but is not installed by default on GitHub's
	# hosted Ubuntu runners. Worth installing manually?
	wasm-pack test --headless --chrome -- --features browser
