# Linux compatibility

GitButler should run well on most modern Linux distributions, but there are too many software and hardware combinations for the core team to support on its own. As such, the GitButler project maintains official support for a select few Linux distributions, and otherwise relies on community-maintained packages.

This document outlines the current level of commitment toward Linux distributions and packaging formats.

> A more dynamic overview of the current state of Linux compatibility can be found in https://github.com/gitbutlerapp/gitbutler/issues/8411

## Official support

The officially supported way to install GitButler is with the `deb` package provided on the [downloads page](https://gitbutler.com/downloads). This package is regularly tested to work well with the following distributions.

- Ubuntu 22.04 LTS (jammy)
- Ubuntu 24.04 LTS (noble)

On these distributions, we aim to provide as good a user experience as on Windows and macOS. Compatibility is routinely verified and compatibility issues are the domain of the core GitButler team. We know that the current user experience does not quite deliver on all fronts, and work on improvements is underway.

This does not mean that the GitButler project is unconcerned with the woes of other Linux distributions. Reports of compatibility issues with _any_ distribution are welcome in the [issue tracker](https://github.com/gitbutlerapp/gitbutler/issues), but with the caveat that such issues are not guaranteed to get prioritized anytime soon. If there is a compatibility trade-off to be made, it will always be made in favor of the officially supported distributions.

## Experimental distribution: `rpm`

We provide an experimental `rpm` package alongside the `deb` package. It is fundamentally the same thing as the `deb` package and should have the same level of compatibility, but it is not regularly tested on any distribution as part of the development process. We provide it as it is no extra effort to build with the current toolchain, and have no reason to believe it would come with any particular compatibility caveats.

## Experimental distribution: AppImage

We provide an experimental AppImage that bundles the core dependencies required to run GitButler. The intention is that it should work on most Linux distributions, but experience shows that compatibility is relatively poor.

The AppImage may be removed in the future if compatibility remains poor.

## Community-maintained distributions

There are several community-maintained distributions of GitButler. Issues with these distributions should typically be brought to the attention of their respective maintainers.

> Know of one we missed? Submit a PR to keep us in the loop!

- Arch Linux User Repository (AUR)
  - [gitbutler](https://aur.archlinux.org/packages/gitbutler)
  - [gitbutler-bin](https://aur.archlinux.org/packages/gitbutler-bin)
- Flatpak
  - [GitButler](https://flathub.org/en/apps/com.gitbutler.gitbutler)
