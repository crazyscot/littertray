# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [1.0.0](https://github.com/crazyscot/littertray/compare/v0.2.1...v1.0.0)

### 🏗️ Build, packaging & CI

- Set up cargo deny - ([6439996](https://github.com/crazyscot/littertray/commit/643999644d17dda26ea05e72947b115aa2d86a57))

### ⚙️ Miscellaneous Tasks

- Chore(ci)(deps): bump codecov/codecov-action from 5.4.3 to 5.5.0 - ([f4b1f3c](https://github.com/crazyscot/littertray/commit/f4b1f3ce6e2abcce3503ac46dd69f37e322ed72e))


## [0.2.1](https://github.com/crazyscot/littertray/compare/v0.2.0...v0.2.1)

### ⚙️ Miscellaneous Tasks

- _(ci)_ Overhaul CI - ([584fa5e](https://github.com/crazyscot/littertray/commit/584fa5eaab71df7b9496b7ea1c5370c28652407b))
- _(ci,deps)_: bump codecov/codecov-action from 5.4.2 to 5.4.3 - ([2ccb4a9](https://github.com/crazyscot/littertray/commit/2ccb4a9f1c8c44d9fa2e1e3210b02d64b41b2152))

## [0.2.0](https://github.com/crazyscot/littertray/compare/v0.1.0...v0.2.0)

### 🐛 Bug Fixes

- [**breaking**] Overhaul locking soundness, return types - ([e09294a](https://github.com/crazyscot/littertray/commit/e09294a1ee0f7b994081d4785ffc8f70c057217b))

### 🏗️ Build, packaging & CI

- Allow building on rust 1.81, without the `async` feature - ([b608f82](https://github.com/crazyscot/littertray/commit/b608f8261305ae76d2859f924ce928b553775199))
- Include doctests in code coverage - ([c4013d8](https://github.com/crazyscot/littertray/commit/c4013d8397d87ab29d8d9d95f64216cae3122275))

### ⚙️ Miscellaneous Tasks

- Enable all features on rust-analyzer, resolve warnings - ([4781ac9](https://github.com/crazyscot/littertray/commit/4781ac90df62c7874f0da7236f162597249a1c3c))
- Fix badges on README - ([0c98f15](https://github.com/crazyscot/littertray/commit/0c98f1554dfa01233e30d0af1c25b63af5fdd69a))

## [0.1.0]

### ⛰️ Features

- Support passthrough(ish) errors from closures - ([63a6291](https://github.com/crazyscot/littertray/commit/63a6291022a067f033630b0a15124071c29f0a83))
- Support variable return values - ([e5289d5](https://github.com/crazyscot/littertray/commit/e5289d557845fc61d7ae7e7ff07ad2480a1efe14))
- Initial commit - ([5b8917a](https://github.com/crazyscot/littertray/commit/5b8917a590c01a055658de9587e1ddab3e16a3cf))

### 🏗️ Build, packaging & CI

- Initial CI workflows - ([53c61f4](https://github.com/crazyscot/littertray/commit/53c61f4320b714dce0ed4dc6961f6e27ffa4bc12))
- Put asyncs behind a feature flag - ([b7cf2bb](https://github.com/crazyscot/littertray/commit/b7cf2bbcb9fab2e3234da1b7996e0c27ce2704df))

### ⚙️ Miscellaneous Tasks

- Determine MSRV and edition; set up CI to suit - ([066f71a](https://github.com/crazyscot/littertray/commit/066f71ac0a0af80571700869f2d01ad4b87c1dc1))
- Add dependabot & release-plz config - ([18f64f1](https://github.com/crazyscot/littertray/commit/18f64f166194e6242806d696105ef829232bb59a))
