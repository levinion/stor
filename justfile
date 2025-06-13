build:
  cargo install --path .

competitions:
  sudo install -Dm 644 ./completions/zsh/_stor /usr/share/zsh/site-functions/

install:
  just build
  just competitions

uninstall:
  rm $HOME/.cargo/bin/stor
  sudo rm /usr/share/zsh/site-functions/_stor

