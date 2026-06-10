# Skip generating debuginfo/debugsource subpackages — cargo's release
# binaries already strip debug info, so these would be empty anyway.
%global debug_package %{nil}

Name:           glycin
Version:        2.1.1
Release:        100.bc7fix3%{?bc7fix_rev}%{?dist}
Summary:        Sandboxed image rendering (with extended DDS BC4-BC7 support)

License:        MPL-2.0 OR LGPL-2.1-or-later
URL:            https://github.com/phantomcortex/glycin
Source0:        %{name}-%{version}.tar.xz

BuildRequires:  cargo
BuildRequires:  rust >= 1.92
BuildRequires:  gcc
BuildRequires:  git-core
BuildRequires:  meson
BuildRequires:  vala
BuildRequires:  pkgconfig
BuildRequires:  pkgconfig(gio-2.0) >= 2.60
BuildRequires:  pkgconfig(gobject-introspection-1.0)
BuildRequires:  pkgconfig(gtk4) >= 4.16
BuildRequires:  pkgconfig(libheif) >= 1.20.0
BuildRequires:  pkgconfig(libjxl) >= 0.11.0
BuildRequires:  pkgconfig(librsvg-2.0) >= 2.52.0
BuildRequires:  pkgconfig(lcms2) >= 2.14
BuildRequires:  pkgconfig(libseccomp) >= 2.5.0

%description
Sandboxed and extendable image decoding. This build adds support for
the DDS BC4, BC5, BC6H, and BC7 compression formats in the image-rs
loader.


%package loaders
Summary:        Sandboxed image rendering (image loading backends)
Requires:       bubblewrap
Obsoletes:      glycin-loaders%{_isa} < %{version}-%{release}
Provides:       glycin-loaders%{_isa} = %{version}-%{release}

%description loaders
Sandboxed and extendable image decoding.

This package contains the different image loading backends. This build
adds DDS BC4/BC5/BC6H/BC7 support to the image-rs loader.


%package thumbnailer
Summary:        Sandboxed image rendering (thumbnailer)
Requires:       glycin-loaders%{_isa} = %{version}-%{release}
Obsoletes:      gdk-pixbuf2 < 2.43.5-1
Obsoletes:      glycin-thumbnailer%{_isa} < %{version}-%{release}
Provides:       glycin-thumbnailer%{_isa} = %{version}-%{release}

%description thumbnailer
Sandboxed and extendable image decoding.

This package contains the thumbnailer implementation.


%package libs
Summary:        Sandboxed image rendering (C library)
Requires:       glycin-loaders%{_isa} = %{version}-%{release}
Obsoletes:      glycin-libs%{_isa} < %{version}-%{release}
Provides:       glycin-libs%{_isa} = %{version}-%{release}

%description libs
Sandboxed and extendable image decoding.

This package contains a shared library interface for glycin.


%package gtk4-libs
Summary:        Sandboxed image rendering (GTK4 integration)
Requires:       glycin-libs%{_isa} = %{version}-%{release}
Obsoletes:      glycin-gtk4-libs%{_isa} < %{version}-%{release}
Provides:       glycin-gtk4-libs%{_isa} = %{version}-%{release}

%description gtk4-libs
Sandboxed and extendable image decoding.

This package contains a shared library interface for glycin
which provides integration with GDK / GTK4.


%package devel
Summary:        Sandboxed image rendering (development files)
Requires:       glycin-libs%{_isa} = %{version}-%{release}

%description devel
Sandboxed and extendable image decoding.

This package contains files for developing against libglycin.


%package gtk4-devel
Summary:        Sandboxed image rendering (GTK4 development files)
Requires:       glycin-devel%{_isa} = %{version}-%{release}
Requires:       glycin-gtk4-libs%{_isa} = %{version}-%{release}

%description gtk4-devel
Sandboxed and extendable image decoding.

This package contains files for developing against libglycin-gtk4.


%prep
%autosetup -n %{name}-%{version}


%build
export CARGO_HOME="%{_builddir}/cargo-home"
mkdir -p "$CARGO_HOME"

%meson \
    -Dprofile=release \
    -Dglycin-loaders=true \
    -Dlibglycin=true \
    -Dlibglycin-gtk4=true \
    -Dglycin-thumbnailer=true \
    -Dintrospection=true \
    -Dvapi=true \
    -Dloaders=glycin-heif,glycin-image-rs,glycin-jxl,glycin-svg \
    -Dtests=false \
    -Dtest_skip_install=true \
    %{nil}

%meson_build


%install
%meson_install


%files loaders
%license LICENSE
%license LICENSE-LGPL-2.1
%license LICENSE-MPL-2.0
%doc README.md
%doc NEWS
%{_libexecdir}/glycin-loaders/
%{_datadir}/glycin-loaders/

%files thumbnailer
%{_bindir}/glycin-thumbnailer
%dir %{_datadir}/thumbnailers/
%{_datadir}/thumbnailers/*.thumbnailer

%files libs
%{_libdir}/libglycin-2.so.0*
%{_libdir}/girepository-1.0/Gly-2.typelib

%files gtk4-libs
%{_libdir}/libglycin-gtk4-2.so.0*
%{_libdir}/girepository-1.0/GlyGtk4-2.typelib

%files devel
%{_includedir}/glycin-2/
%{_libdir}/libglycin-2.so
%{_libdir}/pkgconfig/glycin-2.pc
%{_datadir}/gir-1.0/Gly-2.gir
%{_datadir}/vala/vapi/glycin-2.deps
%{_datadir}/vala/vapi/glycin-2.vapi

%files gtk4-devel
%{_includedir}/glycin-gtk4-2/
%{_libdir}/libglycin-gtk4-2.so
%{_libdir}/pkgconfig/glycin-gtk4-2.pc
%{_datadir}/gir-1.0/GlyGtk4-2.gir
%{_datadir}/vala/vapi/glycin-gtk4-2.deps
%{_datadir}/vala/vapi/glycin-gtk4-2.vapi


%changelog
* Tue Jun 10 2026 phantom <killawattgamer@gmail.com> - 2.1.1-100.bc7fix3
- Qualify Obsoletes/Provides with %%{_isa} so the x86_64 build no longer
  blocks installation of the upstream glycin-libs.i686 needed by steam.i686

* Fri Jun 06 2026 phantom <killawattgamer@gmail.com> - 2.1.1-100.bc7fix2
- Add Obsoletes/Provides to runtime subpackages so plain dnf install
  replaces the upstream glycin without --allowerasing removing Steam

* Thu May 28 2026 phantom <killawattgamer@gmail.com> - 2.1.1-100.bc7fix1
- Add BC4/BC5/BC6H/BC7 DDS decoder support to glycin-image-rs
- Drop-in replacement for the Fedora glycin packages
