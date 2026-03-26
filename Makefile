install: build
	sudo install -Dm 755 ./target/release/stor /usr/bin/
	sudo install -Dm 644 ./completions/zsh/_stor /usr/share/zsh/site-functions/

build:
	cargo build --release

uninstall:
	sudo rm /usr/bin/stor
	sudo rm /usr/share/zsh/site-functions/_stor
