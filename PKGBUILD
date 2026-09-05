# Maintainer: M-MERTS <mxmerts33@gmail.com>
pkgname=certificate-manager
pkgver=2.0.0
pkgrel=1
pkgdesc="Interactive terminal certificate manager for Arch Linux based systems"
arch=('x86_64')
url="https://github.com/mx-merts/CERTIFICATE-MANAGER"
license=('MIT')
depends=('openssl' 'wget' 'fzf' 'ca-certificates')
makedepends=('cargo')
source=("$pkgname-$pkgver.tar.gz::https://github.com/mx-merts/CERTIFICATE-MANAGER/archive/refs/tags/v$pkgver.tar.gz")
sha256sums=('SKIP')

build() {
    cd "CERTIFICATE-MANAGER-$pkgver"
    cargo build --release --locked
}

package() {
    cd "CERTIFICATE-MANAGER-$pkgver"
    install -Dm755 "target/release/certificate-manager" "$pkgdir/usr/bin/certificate-manager"
    install -Dm644 "LICENSE" "$pkgdir/usr/share/licenses/$pkgname/LICENSE"
}
