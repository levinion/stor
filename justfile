build:
  cargo install --path .

competitions:
  sudo install -Dm 644 ./competitions/zsh/_stor /usr/share/zsh/site-functions/

install:
  just build
  just competitions


