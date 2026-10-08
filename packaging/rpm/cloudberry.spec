%global debug_package %{nil}

Name:           cloudberry
Version:        0.1.0
Release:        1%{?dist}
Summary:        Unofficial desktop client for YouTube Music
License:        GPL-3.0-or-later
URL:            https://github.com/nishiki001/cloudberry
Source0:        %{url}/archive/v%{version}/cloudberry-%{version}.tar.gz
# cargo vendor output, attached to every release by the release workflow
Source1:        %{url}/releases/download/v%{version}/cloudberry-vendor.tar.xz

BuildRequires:  cargo
BuildRequires:  rust
BuildRequires:  gcc
BuildRequires:  gcc-c++
BuildRequires:  cmake
BuildRequires:  pkgconfig(alsa)
BuildRequires:  pkgconfig(fontconfig)
BuildRequires:  pkgconfig(xkbcommon)
BuildRequires:  pkgconfig(wayland-client)
BuildRequires:  desktop-file-utils
BuildRequires:  libappstream-glib
Requires:       alsa-lib
Requires:       fontconfig
Requires:       libxkbcommon

%description
Cloudberry is a fast, native desktop music player in the style of the classic
players. It is an unofficial client for YouTube Music and is not affiliated
with Google. yt-dlp and deno are downloaded on first start (and yt-dlp is kept
up to date) into the user's data directory.

%prep
%autosetup -n cloudberry-%{version}
tar -xJf %{SOURCE1}
mkdir -p .cargo
cat > .cargo/config.toml <<'EOF'
[source.crates-io]
replace-with = "vendored-sources"
[source.vendored-sources]
directory = "vendor"
EOF

%build
# the distro -O2 CFLAGS break aws-lc-sys (jitterentropy must be built with -O0)
unset CFLAGS CXXFLAGS
cargo build --release --offline --locked

%install
install -Dm755 target/release/cloudberry %{buildroot}%{_bindir}/cloudberry
install -Dm644 packaging/io.github.nishiki001.Cloudberry.desktop %{buildroot}%{_datadir}/applications/io.github.nishiki001.Cloudberry.desktop
install -Dm644 packaging/io.github.nishiki001.Cloudberry.metainfo.xml %{buildroot}%{_metainfodir}/io.github.nishiki001.Cloudberry.metainfo.xml
for n in 16 22 24 32 48 64 128 256 512; do
  install -Dm644 assets/icons/app/png/cloudberry-$n.png %{buildroot}%{_datadir}/icons/hicolor/${n}x${n}/apps/io.github.nishiki001.Cloudberry.png
done

%check
desktop-file-validate %{buildroot}%{_datadir}/applications/io.github.nishiki001.Cloudberry.desktop
appstream-util validate-relax --nonet %{buildroot}%{_metainfodir}/io.github.nishiki001.Cloudberry.metainfo.xml || :

%files
%license LICENSE
%doc README.md
%{_bindir}/cloudberry
%{_datadir}/applications/io.github.nishiki001.Cloudberry.desktop
%{_metainfodir}/io.github.nishiki001.Cloudberry.metainfo.xml
%{_datadir}/icons/hicolor/*/apps/io.github.nishiki001.Cloudberry.png

%changelog
* Thu Oct 08 2026 nishiki001 <nishiki001@users.noreply.github.com> - 0.1.0-1
- First release
