created ssh key and added to github
sudo curl https://sh.rustup.rs -sSf | sh -s -- -y --default-toolchain=1.55.0
sudo apt-get install openssl libssl-dev gcc
export OPENSSL_LIB_DIR=/usr/lib/x86_64-linux-gnu
export OPENSSL_INCLUDE_DIR=/usr/include/openssl

cargo run --example provisioning -- get-system-date-and-time --uri=http://localhost:8899