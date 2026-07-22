---
title: Open Source
description: Alchemist is licensed under AGPLv3. There is no commercial tier, no paywalled features, and no telemetry on by default. Source, issues, and releases are on GitHub.
keywords:
  - open source transcoding
  - agpl transcoder
  - foss media tools
  - self-hosted transcoding open source
---

Alchemist is licensed under
[AGPLv3](https://github.com/bybrooklyn/alchemist/blob/master/LICENSE).
That means:

- **Free to use**, for any purpose, including commercial.
- **Free to modify.** Fork it, patch it, ship your own
  variant.
- **Free to redistribute**, as long as derivative works stay
  under AGPLv3 — including when you run a modified version as
  a network service.

The entire codebase lives in one repository:
[github.com/bybrooklyn/alchemist](https://github.com/bybrooklyn/alchemist).
There is no private, closed, or "pro" repository that ships
features the public version lacks. The public source tree is
the product.

## What "actually open source" means here

Several tools in this category ship a free tier alongside a
paid tier with extra features, license-key unlocks, or
source-available terms that are not the same thing as a
copyleft open-source project. Alchemist does neither:

- There is no paid tier. Every feature in Alchemist is in
  the AGPLv3 source tree.
- There is no account, no license key, no phone-home check.
- The binary you install is built from the same code in the
  repository you can read.
- Commercial use is allowed under AGPLv3. If you distribute a
  modified build — or offer a modified Alchemist to users over
  a network — you keep it AGPLv3 and ship the source.

That is the line Alchemist draws: no "community edition"
that exists to upsell the real one, and no operational
feature held behind a subscription.

## Documentation license

The human-authored manual is copyright Brooklyn Halmstad and licensed under
[CC BY-SA 4.0](https://creativecommons.org/licenses/by-sa/4.0/). It is
published at [Dead Signal Works](https://deadsignal.works/alchemist/docs/) from
the authoritative Markdown in this repository. The documentation license does
not change the AGPL license on Alchemist software or grant rights to Dead
Signal Works branding and website implementation.

## Telemetry

Opt-in, off by default. The config field is
`system.enable_telemetry` and the setup wizard asks
explicitly. See
[Configuration Reference](/configuration-reference#system).

## Contributing

Bug reports and pull requests are welcome on GitHub:

- [Issues](https://github.com/bybrooklyn/alchemist/issues)
- [Releases](https://github.com/bybrooklyn/alchemist/releases)
- [Source](https://github.com/bybrooklyn/alchemist)

See [Contributing](/contributing/overview) for development
setup.

## FAQ

**Can I use Alchemist commercially?**
Yes. AGPLv3 permits commercial use. Derivative works you
distribute — or run as a network service — must remain under
AGPLv3 and ship their source.

**Is there a paid or enterprise tier?**
No. There is no closed source, no private feature set, no
license key system, and no intent to add one.

**Do I have to share my modifications?**
Yes if you distribute a modified build, **and also** if you
make a modified Alchemist available to users over a network —
AGPLv3 section 13 requires you to offer those users the
modified source. Genuinely private, single-operator use with
no other network users does not require publication.
See the [AGPLv3 text](https://www.gnu.org/licenses/agpl-3.0.html)
for the specifics.

**Does Alchemist phone home?**
No. Telemetry is opt-in and off by default. The config key
is `system.enable_telemetry`. The setup wizard asks
explicitly.

**Where do I report bugs or request features?**
[GitHub Issues](https://github.com/bybrooklyn/alchemist/issues).
Bug reports with reproduction steps and log excerpts get
triaged first.

## Comparisons

If you're evaluating Alchemist against another tool and
licensing matters to you:

- [Alternatives overview](/alternatives/)
- [Alchemist vs Tdarr](/alternatives/tdarr)
- [Alchemist vs FileFlows](/alternatives/fileflows)
