# Changelog

All notable changes to this project will be documented in this file.

## [0.22.2](https://github.com/aschey/stream-download-rs/compare/stream-download-v0.22.1..stream-download-v0.22.2) - 2025-07-17

### Documentation

- Add link to blog post ([#207](https://github.com/aschey/stream-download-rs/issues/207)) - ([c987899](https://github.com/aschey/stream-download-rs/commit/c987899cb52e3ff8fdf2780aed8c2b678d783925))

## [0.22.1](https://github.com/aschey/stream-download-rs/compare/stream-download-v0.22.0..stream-download-v0.22.1) - 2025-06-19

### Features

- Prevent re-requesting previously downloaded stream portions during seek operations ([#205](https://github.com/aschey/stream-download-rs/issues/205)) - ([03f645a](https://github.com/aschey/stream-download-rs/commit/03f645aea81eb5455978792b6d3eb6bb7f90074e))

## [0.22.0](https://github.com/aschey/stream-download-rs/compare/stream-download-v0.21.1..stream-download-v0.22.0) - 2025-06-19

### Refactor

- [**breaking**] Remove seek_buffer_size ([#203](https://github.com/aschey/stream-download-rs/issues/203)) - ([66a6320](https://github.com/aschey/stream-download-rs/commit/66a6320191bca92e207a0aadd91ef05ab382f29f))

## [0.21.1](https://github.com/aschey/stream-download-rs/compare/stream-download-v0.21.0..stream-download-v0.21.1) - 2025-06-07

### Miscellaneous Tasks

- Update Cargo.toml dependencies - ([0000000](https://github.com/aschey/stream-download-rs/commit/0000000))

## [0.21.0](https://github.com/aschey/stream-download-rs/compare/stream-download-v0.20.0..stream-download-v0.21.0) - 2025-05-26

### Features

- Batch large writes ([#198](https://github.com/aschey/stream-download-rs/issues/198)) - ([018cd4f](https://github.com/aschey/stream-download-rs/commit/018cd4f72d5619c2cd9e504b96b95ee7ebb13d81))
- [**breaking**] Require nonzero seek buffer size ([#200](https://github.com/aschey/stream-download-rs/issues/200)) - ([05b621c](https://github.com/aschey/stream-download-rs/commit/05b621cd7b58645a3015fa65140aa40b2f49b452))

## [0.20.0](https://github.com/aschey/stream-download-rs/compare/stream-download-v0.19.1..stream-download-v0.20.0) - 2025-05-16

### Miscellaneous Tasks

- [**breaking**] Bump MSRV to 1.82 ([#193](https://github.com/aschey/stream-download-rs/issues/193)) - ([a9de53d](https://github.com/aschey/stream-download-rs/commit/a9de53de117521ff175dbb41b0eab64863b89dec))
- [**breaking**] Bump MSRV to 1.85 ([#197](https://github.com/aschey/stream-download-rs/issues/197)) - ([f8c65c8](https://github.com/aschey/stream-download-rs/commit/f8c65c869663beafe787efa9dec773f7de1de2aa))

### Refactor

- Simplify ring buffer io::read implementation ([#195](https://github.com/aschey/stream-download-rs/issues/195)) - ([034c549](https://github.com/aschey/stream-download-rs/commit/034c5497ed8926ed03f65780dd4533ab2d73b538))
- Apply clippy lints from Rust 1.87 ([#196](https://github.com/aschey/stream-download-rs/issues/196)) - ([f7a8e5f](https://github.com/aschey/stream-download-rs/commit/f7a8e5f5024ffbc5be6dd261268b0c21c5c44c91))

## [0.19.1](https://github.com/aschey/stream-download-rs/compare/stream-download-v0.19.0..stream-download-v0.19.1) - 2025-05-04

### Documentation

- Remove old link from readme ([#191](https://github.com/aschey/stream-download-rs/issues/191)) - ([60becb1](https://github.com/aschey/stream-download-rs/commit/60becb180c3c28d3222e88873b2d307a7ea08b83))

### Miscellaneous Tasks

- Remove deps.rs badge ([#188](https://github.com/aschey/stream-download-rs/issues/188)) - ([d4bdcab](https://github.com/aschey/stream-download-rs/commit/d4bdcab0ac350fa653617a347866f4b675edf2de))

## [0.19.0](https://github.com/aschey/stream-download-rs/compare/stream-download-v0.18.1..stream-download-v0.19.0) - 2025-04-09

### Features

-  [**breaking**] Flexible bounded and unbounded storage types for adaptive provider ([#185](https://github.com/aschey/stream-download-rs/issues/185)) - ([e00c2b8](https://github.com/aschey/stream-download-rs/commit/e00c2b8c3371f9a2dacb176c2efbbbdffda291e2))

## [0.18.1](https://github.com/aschey/stream-download-rs/compare/stream-download-v0.18.0..stream-download-v0.18.1) - 2025-04-06

### Miscellaneous Tasks

- Apply workspace lints ([#184](https://github.com/aschey/stream-download-rs/issues/184)) - ([2b997db](https://github.com/aschey/stream-download-rs/commit/2b997dbab4b1b4b33615410995c7fe68ef2f0ddf))

## [0.18.0](https://github.com/aschey/stream-download-rs/compare/v0.17.0..stream-download-v0.18.0) - 2025-04-05

### Refactor

- [**breaking**] Move OpenDAL integration into separate crate ([#174](https://github.com/aschey/stream-download-rs/issues/174)) - ([987cc15](https://github.com/aschey/stream-download-rs/commit/987cc15f5307df96598d3f4d13bb04409d2b7dcf))

### Dependencies

- *(deps)* replace "futures" with "futures-util" ([#174](https://github.com/aschey/stream-download-rs/issues/176)) - ([3a041f5](https://github.com/aschey/stream-download-rs/commit/3a041f55e956c0d04042ec6a3e17a852cc8a3724))

## [0.17.0](https://github.com/aschey/stream-download-rs/compare/v0.16.1..v0.17.0) - 2025-03-31

### Refactor

- [**breaking**] Apply additional clippy lints ([#171](https://github.com/aschey/stream-download-rs/issues/171)) - ([60e066a](https://github.com/aschey/stream-download-rs/commit/60e066a3f0320e1451ad09ee55923a6e65ab68b9))

<!-- generated by git-cliff -->
## [0.16.1](https://github.com/aschey/stream-download-rs/compare/v0.16.0..v0.16.1) - 2025-03-23

### Features

- More customization for command args ([#168](https://github.com/aschey/stream-download-rs/issues/168)) - ([eee963d](https://github.com/aschey/stream-download-rs/commit/eee963da2aa1a483dc2974db396f2ee3d4322be3))

### Miscellaneous Tasks

- Fix deprecation warning in s3 example ([#164](https://github.com/aschey/stream-download-rs/issues/164)) - ([9bc5715](https://github.com/aschey/stream-download-rs/commit/9bc5715a16e84790e8cea17842d551ee37f5c1da))

<!-- generated by git-cliff -->
## [0.16.0](https://github.com/aschey/stream-download-rs/compare/v0.15.1..v0.16.0) - 2025-03-10

### Dependencies

- *(deps)* [**breaking**] Update opendal requirement from 0.51 to 0.52 ([#161](https://github.com/aschey/stream-download-rs/issues/161)) - ([16e44be](https://github.com/aschey/stream-download-rs/commit/16e44bec64bca1cc909eecc9ffd294effb66973d))
- *(deps)* Update ctor requirement from 0.3.3 to 0.4.0 ([#160](https://github.com/aschey/stream-download-rs/issues/160)) - ([7632294](https://github.com/aschey/stream-download-rs/commit/76322942c6841d8811a74d242b5323ac03b8c865))
- *(deps)* Update rstest requirement from 0.24.0 to 0.25.0 ([#163](https://github.com/aschey/stream-download-rs/issues/163)) - ([fc375f8](https://github.com/aschey/stream-download-rs/commit/fc375f849c7c0ce1ef5e67a29ee55b622e9139e7))

### Refactor

- [**breaking**] Bump MSRV to 1.80 ([#157](https://github.com/aschey/stream-download-rs/issues/157)) - ([cd0a0cb](https://github.com/aschey/stream-download-rs/commit/cd0a0cb1268cd82c1f218ba4ab6e78eb9f045b64))
- [**breaking**] Bump MSRV to 1.81 ([#162](https://github.com/aschey/stream-download-rs/issues/162)) - ([83bbdbd](https://github.com/aschey/stream-download-rs/commit/83bbdbd4829df736e14f4660188b1896b82a5ccd))

<!-- generated by git-cliff -->
## [0.15.1](https://github.com/aschey/stream-download-rs/compare/v0.15.0..v0.15.1) - 2025-02-15

### Bug Fixes

- Format sorting for yt-dlp examples (#156) - ([806be91](https://github.com/aschey/stream-download-rs/commit/806be913ee1502a500ea57a66b48688482d9b8c9))

### Dependencies

- *(deps)* Update ctor requirement from 0.2.7 to 0.3.3 (#154) - ([628b62d](https://github.com/aschey/stream-download-rs/commit/628b62df4494db3dd86f4cc896ee6a279621f4aa))

<!-- generated by git-cliff -->
## [0.15.0](https://github.com/aschey/stream-download-rs/compare/v0.14.1..v0.15.0) - 2025-02-15

### Features

- Add method to retrieve content length from StreamDownload (#152) - ([4778348](https://github.com/aschey/stream-download-rs/commit/4778348f56cf6ebec771e3dacba29e8514cf7e16))

### Miscellaneous Tasks

- Doc updates (#149) - ([2feb7cd](https://github.com/aschey/stream-download-rs/commit/2feb7cd4c545b3989e27d456dc082c17cec7ca1e))

### Refactor

- [**breaking**] Rename get_cancellation_token to cancellation_token (#153) - ([cd13d91](https://github.com/aschey/stream-download-rs/commit/cd13d911bcefedc80f1371bb6c43bd0f97b813c4))

<!-- generated by git-cliff -->
## [0.14.1](https://github.com/aschey/stream-download-rs/compare/v0.14.0..v0.14.1) - 2025-01-18

### Bug Fixes

- HTTP range end value should be inclusive (#148) - ([29ee4ee](https://github.com/aschey/stream-download-rs/commit/29ee4ee2eec61741dcf01126bc9ab6c28e8b13f1))

### Dependencies

- *(deps)* Update rstest requirement from 0.23.0 to 0.24.0 (#146) - ([54e3edb](https://github.com/aschey/stream-download-rs/commit/54e3edb8db707b4c3fb4e02a4694786d367cc300))
- *(deps)* Update axum requirement from 0.7.5 to 0.8.1 (#145) - ([d4a89d0](https://github.com/aschey/stream-download-rs/commit/d4a89d0b6e5bfa8bb962667b761dc14793310907))

<!-- generated by git-cliff -->
## [0.14.0](https://github.com/aschey/stream-download-rs/compare/v0.13.3..v0.14.0) - 2024-12-24

### Dependencies

- *(deps)* [**breaking**] Update opendal to 0.51 (#143) - ([1901eee](https://github.com/aschey/stream-download-rs/commit/1901eee78c7d315efd933933a58affaa0ae70ca7))

<!-- generated by git-cliff -->
## [0.13.3](https://github.com/aschey/stream-download-rs/compare/v0.13.2..v0.13.3) - 2024-12-23

### Dependencies

- *(deps)* Update regex to 1.5.5 (#141) - ([fc8830b](https://github.com/aschey/stream-download-rs/commit/fc8830b7feeb0a79c36a0cebe36c9fa91a3e29e4))

<!-- generated by git-cliff -->
## [0.13.2](https://github.com/aschey/stream-download-rs/compare/v0.13.1..v0.13.2) - 2024-12-08

### Bug Fixes

- Ensure read and seek operations don't go past the end of the stream (#137) - ([065e368](https://github.com/aschey/stream-download-rs/commit/065e368ddf5730ae8f6fa60afafa9768a47db528))

### Features

- Mark StreamDownload as UnwindSafe (#139) - ([30fd101](https://github.com/aschey/stream-download-rs/commit/30fd1012dfacd2820e711dfb1b659c4007e9b0eb))
- Add video playback example (#140) - ([6c34a46](https://github.com/aschey/stream-download-rs/commit/6c34a46f711751e8ad1ab209827608f942bd3db9))

<!-- generated by git-cliff -->
## [0.13.1](https://github.com/aschey/stream-download-rs/compare/v0.13.0..v0.13.1) - 2024-11-24

### Features

- Add registry for handling different source types ([#135](https://github.com/aschey/stream-download-rs/pull/135)) - ([b8384b6](https://github.com/aschey/stream-download-rs/commit/b8384b617ff1e21c7eef1fb8b8e03ad04a16a24e))

<!-- generated by git-cliff -->
## [0.13.0](https://github.com/aschey/stream-download-rs/compare/v0.12.0..v0.13.0) - 2024-11-20

### Features

- [**breaking**] Revert default stream behavior to cancel on drop ([#132](https://github.com/aschey/stream-download-rs/pull/132)) - ([61cd6cc](https://github.com/aschey/stream-download-rs/commit/61cd6ccfef874c641e42913dc99d4b2b5d7d97a4))

<!-- generated by git-cliff -->
## [0.12.0](https://github.com/aschey/stream-download-rs/compare/v0.11.2..v0.12.0) - 2024-11-14

### Bug Fixes

- *(examples)* Create decoder and sink on the same thread ([#123](https://github.com/aschey/stream-download-rs/pull/123)) - ([5065e78](https://github.com/aschey/stream-download-rs/commit/5065e78e69b808c9106dcedd42cb6a86648afa60))
- Fix race condition when reporting stream errors ([#127](https://github.com/aschey/stream-download-rs/pull/127)) - ([594d41d](https://github.com/aschey/stream-download-rs/commit/594d41d019cf839d97773c65a222ea1e728da967))

### Dependencies

- *(deps)* Allow reqwest-middleware 0.3.-0.4 ([#130](https://github.com/aschey/stream-download-rs/pull/130)) - ([c0bc067](https://github.com/aschey/stream-download-rs/commit/c0bc067994963fa285c83edfd78d733b4c4e219a))

### Features

- Add setting to cancel stream on drop ([#124](https://github.com/aschey/stream-download-rs/pull/124)) - ([785d54e](https://github.com/aschey/stream-download-rs/commit/785d54e370e9d81dbadc2c1cf7a0528ba84d4911))
- Add stream handle ([#125](https://github.com/aschey/stream-download-rs/pull/125)) - ([6ae481b](https://github.com/aschey/stream-download-rs/commit/6ae481b7dfeadaff712d4297ce42d5b8883364aa))
- [**breaking**] Add SourceStream implementation for AsyncRead ([#126](https://github.com/aschey/stream-download-rs/pull/126)) - ([bac8c64](https://github.com/aschey/stream-download-rs/commit/bac8c6445439d9086b7bf1ba0130042882df7f3a))
- [**breaking**] Add SourceStream implementation for processes ([#128](https://github.com/aschey/stream-download-rs/pull/128)) - ([91f562c](https://github.com/aschey/stream-download-rs/commit/91f562c314d1500d020b6f95813f36dfc90fb692))
- Add yt-dlp command wrappers ([#129](https://github.com/aschey/stream-download-rs/pull/129)) - ([4213bf7](https://github.com/aschey/stream-download-rs/commit/4213bf70bfc6a640226cdb01b1f4660cda9771a7))

### Refactor

- Separate some large source files ([#121](https://github.com/aschey/stream-download-rs/pull/121)) - ([aabbab8](https://github.com/aschey/stream-download-rs/commit/aabbab8aaae078ef0c5979bebf49bc4f6eae8bf8))

<!-- generated by git-cliff -->
## [0.11.2](https://github.com/aschey/stream-download-rs/compare/v0.11.1..v0.11.2) - 2024-11-09

### Dependencies

- *(deps)* Update rodio requirement from 0.19.0 to 0.20.1 ([#118](https://github.com/aschey/stream-download-rs/pull/118)) - ([2b27b77](https://github.com/aschey/stream-download-rs/commit/2b27b77d7705f24ae148800797ebd12f270adbb6))
- *(deps)* Update thiserror requirement from 1.0.21 to 2.0.1 ([#119](https://github.com/aschey/stream-download-rs/pull/119)) - ([0741fcc](https://github.com/aschey/stream-download-rs/commit/0741fcc96f607ba1d7e62248bbea1e66cb770aaa))

<!-- generated by git-cliff -->
## [0.11.1](https://github.com/aschey/stream-download-rs/compare/v0.11.0..v0.11.1) - 2024-11-03

### Features

- Add integration with reqwest-middleware ([#116](https://github.com/aschey/stream-download-rs/pull/116)) - ([a2acbb2](https://github.com/aschey/stream-download-rs/commit/a2acbb2fe3d84552bd449f4783f26ae221763b60))

<!-- generated by git-cliff -->
## [0.11.0](https://github.com/aschey/stream-download-rs/compare/v0.10.0..v0.11.0) - 2024-11-02

### Features

- Return error when the stream is cancelled by the user ([#111](https://github.com/aschey/stream-download-rs/pull/111)) - ([d37e547](https://github.com/aschey/stream-download-rs/commit/d37e547ab48f8af253a0545d91a71166d222a6d8))
- Allow cancelling the stream within a progress callback ([#113](https://github.com/aschey/stream-download-rs/pull/113)) - ([d74ac82](https://github.com/aschey/stream-download-rs/commit/d74ac824fe2126ed1b92461ecd3e1304423e855a))
- Add reconnect callback ([#114](https://github.com/aschey/stream-download-rs/pull/114)) - ([617d6e7](https://github.com/aschey/stream-download-rs/commit/617d6e7453fa42419d26cbaab60f8944a65c8226))

<!-- generated by git-cliff -->
## [0.10.0](https://github.com/aschey/stream-download-rs/compare/v0.9.0..v0.10.0) - 2024-11-02

### Bug Fixes

- Notify readers on stream failure ([#109](https://github.com/aschey/stream-download-rs/pull/109)) - ([39cd434](https://github.com/aschey/stream-download-rs/commit/39cd4348de2fd26902e163baddb11d8811b23d98))
- Attempt range request even if not explicitly supported ([#110](https://github.com/aschey/stream-download-rs/pull/110)) - ([6a294c5](https://github.com/aschey/stream-download-rs/commit/6a294c5213a3a56ff1d2adc23bf1a139d422435e))

### Dependencies

- *(deps)* Update testcontainers-modules requirement from 0.10.0 to 0.11.1 ([#102](https://github.com/aschey/stream-download-rs/pull/102)) - ([6ca97a3](https://github.com/aschey/stream-download-rs/commit/6ca97a3ecc91bbf5e717a3cf0469a0bbc34b862a))
- *(deps)* Update rstest requirement from 0.22.0 to 0.23.0 ([#105](https://github.com/aschey/stream-download-rs/pull/105)) - ([7af200d](https://github.com/aschey/stream-download-rs/commit/7af200df585a7ff58d7982c1501cee82f893a42e))
- *(deps)* Expand compatible OpenDAL version range ([#106](https://github.com/aschey/stream-download-rs/pull/106)) - ([add3932](https://github.com/aschey/stream-download-rs/commit/add393269551cd3548744f38107ac2b9ef0bbcc6))

### Features

- Error on read or seek when the stream failed to download ([#108](https://github.com/aschey/stream-download-rs/pull/108)) - ([e451153](https://github.com/aschey/stream-download-rs/commit/e4511534503c998901716c6aa0a7d9d196834245))
- Add configurable timeout to retry streams after network failures ([#107](https://github.com/aschey/stream-download-rs/pull/107)) - ([c99bc21](https://github.com/aschey/stream-download-rs/commit/c99bc21f0b4aba40fd8a1f9a1e8119534292a4e1))

<!-- generated by git-cliff -->
## [0.9.0](https://github.com/aschey/stream-download-rs/compare/v0.8.0..v0.9.0) - 2024-09-21

### Dependencies

- *(deps)* [**breaking**] Update opendal requirement from 0.49 to 0.50 ([#96](https://github.com/aschey/stream-download-rs/pull/96)) - ([092707d](https://github.com/aschey/stream-download-rs/commit/092707d3a486ca05786993639fe76bdba86f071a))
- *(deps)* Update tower-http requirement from 0.5.2 to 0.6.0 ([#97](https://github.com/aschey/stream-download-rs/pull/97)) - ([172f11b](https://github.com/aschey/stream-download-rs/commit/172f11b4f34845b3ab881d1a8f96d83a9d925d5b))

### Miscellaneous Tasks

- Update rustfmt to 2024 edition ([#98](https://github.com/aschey/stream-download-rs/pull/98)) - ([e1c614b](https://github.com/aschey/stream-download-rs/commit/e1c614be2d551f79004ec156bbb72ed1e6c46b54))
- Use repo token for release-plz ([#100](https://github.com/aschey/stream-download-rs/pull/100)) - ([b82f5e3](https://github.com/aschey/stream-download-rs/commit/b82f5e32a16d8235116178bc4c02f44c27331a39))

<!-- generated by git-cliff -->
## [0.8.0](https://github.com/aschey/stream-download-rs/compare/v0.7.3..v0.8.0) - 2024-09-08

### Bug Fixes

- Apply the correct offset for SeekFrom::End ([#93](https://github.com/aschey/stream-download-rs/pull/93)) - ([b3237d9](https://github.com/aschey/stream-download-rs/commit/b3237d97b4bd700df28b77d6a33561e4343ad91d))

### Features

- Return specific error types when creating a stream ([#91](https://github.com/aschey/stream-download-rs/pull/91)) - ([13fc7ca](https://github.com/aschey/stream-download-rs/commit/13fc7caf8b72154417fae57bb1500fab894fd1de))
- Add backpressure to handle slow readers when using BoundedStorage ([#94](https://github.com/aschey/stream-download-rs/pull/94)) - ([e9f0472](https://github.com/aschey/stream-download-rs/commit/e9f04723d9b0285969f598b9a75083871222592e))

<!-- generated by git-cliff -->
## [0.7.3](https://github.com/aschey/stream-download-rs/compare/v0.7.2..v0.7.3) - 2024-09-01

### Dependencies

- *(deps)* Update testcontainers-modules requirement from 0.9.0 to 0.10.0 ([#89](https://github.com/aschey/stream-download-rs/pull/89)) - ([5e8dff0](https://github.com/aschey/stream-download-rs/commit/5e8dff0cb22a8796a7cfb3667b3bfd24a5a857a6))
- *(deps)* Update tower requirement from 0.4.13 to 0.5.0 ([#86](https://github.com/aschey/stream-download-rs/pull/86)) - ([be39cd6](https://github.com/aschey/stream-download-rs/commit/be39cd64ad10a4ab22835d2543cbf41437d3eee7))
- *(deps)* Update opendal requirement from 0.48 to 0.49 ([#87](https://github.com/aschey/stream-download-rs/pull/87)) - ([d8e14dc](https://github.com/aschey/stream-download-rs/commit/d8e14dcd360267bc199b1efcb245d0fe0af71b8f))
- *(deps)* Update rstest requirement from 0.21.0 to 0.22.0 ([#88](https://github.com/aschey/stream-download-rs/pull/88)) - ([cc5ba7d](https://github.com/aschey/stream-download-rs/commit/cc5ba7dac32331421761f517d9a4e8ac1417ec2f))

### Documentation

- Fix a few problems with the examples ([#83](https://github.com/aschey/stream-download-rs/pull/83)) - ([d56e621](https://github.com/aschey/stream-download-rs/commit/d56e6214a95b621e74b59cebdde49ebcf0b49c20))

### Miscellaneous Tasks

- Bump EmbarkStudios/cargo-deny-action from 1 to 2 ([#85](https://github.com/aschey/stream-download-rs/pull/85)) - ([657fc21](https://github.com/aschey/stream-download-rs/commit/657fc217246db1ca93ef3b363fef36a7323a0abe))

<!-- generated by git-cliff -->
## [0.7.2](https://github.com/aschey/stream-download-rs/compare/v0.7.1..v0.7.2) - 2024-08-14

### Features

- Add method to get the stream cancellation token ([#80](https://github.com/aschey/stream-download-rs/pull/80)) - ([452239f](https://github.com/aschey/stream-download-rs/commit/452239f1500c67de5aa90c093397c6fa352c56a7))

<!-- generated by git-cliff -->
## [0.7.1](https://github.com/aschey/stream-download-rs/compare/v0.7.0..v0.7.1) - 2024-08-02

### Dependencies

- *(deps)* Update opendal requirement from 0.47 to 0.48 ([#77](https://github.com/aschey/stream-download-rs/pull/77)) - ([bb35f63](https://github.com/aschey/stream-download-rs/commit/bb35f63bec85a1b846897804f0220503be04b554))
- *(deps)* Update testcontainers-modules requirement from 0.8.0 to 0.9.0 ([#78](https://github.com/aschey/stream-download-rs/pull/78)) - ([c30d9a1](https://github.com/aschey/stream-download-rs/commit/c30d9a1406eac1031764080dea4935cb3388753d))

<!-- generated by git-cliff -->
## [0.7.0](https://github.com/aschey/stream-download-rs/compare/v0.6.0..v0.7.0) - 2024-07-28

### Bug Fixes

- Don't pass in RUSTFLAGS when checking docs ([#67](https://github.com/aschey/stream-download-rs/pull/67)) - ([ecfdb0a](https://github.com/aschey/stream-download-rs/commit/ecfdb0adc591711f67e51ee50fc5780cf1e972ed))

### Documentation

- Mention icy-metadata ([#69](https://github.com/aschey/stream-download-rs/pull/69)) - ([4b3c7cd](https://github.com/aschey/stream-download-rs/commit/4b3c7cd067ac373058d9f2d22b5051c326a302c3))

### Features

- Add OpenDAL transport ([#73](https://github.com/aschey/stream-download-rs/pull/73)) - ([c6e0e90](https://github.com/aschey/stream-download-rs/commit/c6e0e90e4b149c5baa4dba788278a7ac65ce82c1))

### Miscellaneous Tasks

- Only check direct minimal versions ([#74](https://github.com/aschey/stream-download-rs/pull/74)) - ([6e5d5d9](https://github.com/aschey/stream-download-rs/commit/6e5d5d9b709a52275d8bd6269007a1564dbfe0c0))
- Update cargo-deny config ([#75](https://github.com/aschey/stream-download-rs/pull/75)) - ([71d5040](https://github.com/aschey/stream-download-rs/commit/71d504006adcad4b4c27bb6c3a883034305b39a6))

<!-- generated by git-cliff -->
## [0.6.0](https://github.com/aschey/stream-download-rs/compare/v0.5.2..v0.6.0) - 2024-07-06

### Bug Fixes

- Don't enable rustls for docs build ([#64](https://github.com/aschey/stream-download-rs/pull/64)) - ([f292373](https://github.com/aschey/stream-download-rs/commit/f2923730de71a70da20cf03d203b397e00ace5a7))

### Dependencies

- *(deps)* Update rstest requirement from 0.18.2 to 0.19.0 ([#56](https://github.com/aschey/stream-download-rs/pull/56)) - ([5c2a08e](https://github.com/aschey/stream-download-rs/commit/5c2a08ecca6448948ef074f735dc8ba519a36414))
- *(deps)* Update rstest requirement from 0.19.0 to 0.20.0 ([#60](https://github.com/aschey/stream-download-rs/pull/60)) - ([3a6666c](https://github.com/aschey/stream-download-rs/commit/3a6666cc4743e353e760757ca6e8c49a001abf72))
- *(deps)* Update rodio requirement from 0.17.3 to 0.18.1 ([#59](https://github.com/aschey/stream-download-rs/pull/59)) - ([b4d6399](https://github.com/aschey/stream-download-rs/commit/b4d63990141f3acf583e2a4c5c6fe4bf6c3c56b8))
- *(deps)* Update rodio requirement from 0.18.1 to 0.19.0 ([#63](https://github.com/aschey/stream-download-rs/pull/63)) - ([4650daf](https://github.com/aschey/stream-download-rs/commit/4650daf5513f59d0a4e869497286c143c7c8877f))
- *(deps)* Update rstest requirement from 0.20.0 to 0.21.0 ([#62](https://github.com/aschey/stream-download-rs/pull/62)) - ([ceb197b](https://github.com/aschey/stream-download-rs/commit/ceb197b192e4adf663fd43d1095a84990ee8de92))

### Features

- Add utility function for creating custom clients ([#52](https://github.com/aschey/stream-download-rs/pull/52)) - ([a305f7f](https://github.com/aschey/stream-download-rs/commit/a305f7f4a215c09f3679f92ea9635ddd276f4437))
- Add stream progress callback ([#65](https://github.com/aschey/stream-download-rs/pull/65)) - ([8c81a1b](https://github.com/aschey/stream-download-rs/commit/8c81a1bcacf2350211c7240c3da524b1ccf5bf01))
- Restart prefetch when seeking into a missing section ([#66](https://github.com/aschey/stream-download-rs/pull/66)) - ([cd11c47](https://github.com/aschey/stream-download-rs/commit/cd11c478afa9a5ee54556d30fb16d3fd6a961e51))

### Miscellaneous Tasks

- Remove committed action ([#53](https://github.com/aschey/stream-download-rs/pull/53)) - ([b01a292](https://github.com/aschey/stream-download-rs/commit/b01a292f26cb134aca351f409ba0b8fde9fbd2bd))
- Add additional clippy lints ([#55](https://github.com/aschey/stream-download-rs/pull/55)) - ([1ab82f9](https://github.com/aschey/stream-download-rs/commit/1ab82f91aceca992818395d58f899ec2121a4882))
- Increase test threads ([#57](https://github.com/aschey/stream-download-rs/pull/57)) - ([f1ebdf4](https://github.com/aschey/stream-download-rs/commit/f1ebdf46e655664b1bf3f8cc9f0dc259d5b26f8b))

<!-- generated by git-cliff -->
## [0.5.2](https://github.com/aschey/stream-download-rs/compare/v0.5.1..v0.5.2) - 2024-04-04

### Bug Fixes

- Handle error statuses from http requests ([#50](https://github.com/aschey/stream-download-rs/pull/50)) - ([a4ec987](https://github.com/aschey/stream-download-rs/commit/a4ec98739d114df2e7c37cd7679e02f61fdadb1f))

<!-- generated by git-cliff -->
## [0.5.1](https://github.com/aschey/stream-download-rs/compare/v0.5.0..v0.5.1) - 2024-03-31

### Bug Fixes

- Only log appropriate seek errors ([#49](https://github.com/aschey/stream-download-rs/pull/49)) - ([b6a900c](https://github.com/aschey/stream-download-rs/commit/b6a900cde093cab486fd886e45a3ca58d9862645))

### Features

- Use native async traits ([#45](https://github.com/aschey/stream-download-rs/pull/45)) - ([3c61e78](https://github.com/aschey/stream-download-rs/commit/3c61e78e0c475688f085d504eaf1e1533df835c9))
- Make seek buffer size configurable ([#48](https://github.com/aschey/stream-download-rs/pull/48)) - ([42783d6](https://github.com/aschey/stream-download-rs/commit/42783d6294f6d6419e04f65d2aac68090412e685))

### Miscellaneous Tasks

- Update reqwest to 0.12 ([#46](https://github.com/aschey/stream-download-rs/pull/46)) - ([ed75e88](https://github.com/aschey/stream-download-rs/commit/ed75e880a89b89bc780fdf1894c1bec228ccb686))

<!-- generated by git-cliff -->
## [0.5.0](https://github.com/aschey/stream-download-rs/compare/v0.4.2..v0.5.0) - 2024-03-20

### Features

- *(storage)* Make tempfile more configurable ([#42](https://github.com/aschey/stream-download-rs/pull/42)) - ([c71de8b](https://github.com/aschey/stream-download-rs/commit/c71de8baeceafa0a43ec5b953741a24f1ac73888))

### Miscellaneous Tasks

- Update dev-dependencies - ([d42514e](https://github.com/aschey/stream-download-rs/commit/d42514e11a70ebd4f39dee90e2fb59c1d1b3e22b))

<!-- generated by git-cliff -->
## [0.4.2](https://github.com/aschey/stream-download-rs/compare/v0.4.1..v0.4.2) - 2024-02-10

### Bug Fixes

- Use proper buffer size with BoundedStorageProvider when content length is known ([#37](https://github.com/aschey/stream-download-rs/pull/37)) - ([66182d8](https://github.com/aschey/stream-download-rs/commit/66182d8c5ee02f3c889cb1076f9f717838a23ae6))
- Ensure content length fits within the requested buffer ([#39](https://github.com/aschey/stream-download-rs/pull/39)) - ([6a15e94](https://github.com/aschey/stream-download-rs/commit/6a15e94ace9d26e489adf676c7d2ad3f919f9ed6))

<!-- generated by git-cliff -->
## [0.4.1](https://github.com/aschey/stream-download-rs/compare/v0.4.0..v0.4.1) - 2024-02-08

### Bug Fixes

- Fix release build ([#34](https://github.com/aschey/stream-download-rs/pull/34)) - ([8cc3163](https://github.com/aschey/stream-download-rs/commit/8cc31632f9783fa6fbe783b2ec1b7adda8e23b93))

<!-- generated by git-cliff -->
## [0.4.0](https://github.com/aschey/stream-download-rs/compare/v0.3.0..v0.4.0) - 2023-12-03

### Bug Fixes

- Set correct msrv in package metadata - ([5760cfe](https://github.com/aschey/stream-download-rs/commit/5760cfec8509cfb7c74b42c484791b69c703bc4b))

### Miscellaneous Tasks

- Fix msrv check - ([9f7e632](https://github.com/aschey/stream-download-rs/commit/9f7e63220c9f37ffa9f5ef5aee4a38dd3bcc5a3f))
- Update git cliff config - ([4feca94](https://github.com/aschey/stream-download-rs/commit/4feca948fd8bc57ae555fb407b45540d505208dd))
- Relax committed lints ([#16](https://github.com/aschey/stream-download-rs/pull/16)) - ([d4884df](https://github.com/aschey/stream-download-rs/commit/d4884dfd0ee6671577b880cef78982ec4b038d90))
- Run clippy on examples ([#22](https://github.com/aschey/stream-download-rs/pull/22)) - ([3501cd1](https://github.com/aschey/stream-download-rs/commit/3501cd1f45315ef0862fa195a4bd52494ec31c8d))
- Bump actions/checkout from 3 to 4 ([#28](https://github.com/aschey/stream-download-rs/pull/28)) - ([68a8b3d](https://github.com/aschey/stream-download-rs/commit/68a8b3d0156fc38282f6f4a1417ce025299a4218))
- Only package rust files - ([613861f](https://github.com/aschey/stream-download-rs/commit/613861f5bb30f8b8eeba2c6d25ba35689d2430b0))

### Refactor

- Reader and writer creation now one method ([#14](https://github.com/aschey/stream-download-rs/pull/14)) - ([aca2585](https://github.com/aschey/stream-download-rs/commit/aca2585718aedefa3f917c7494d72320b735753d))
- Refactor stream internals ([#32](https://github.com/aschey/stream-download-rs/pull/32)) - ([b4d84b5](https://github.com/aschey/stream-download-rs/commit/b4d84b50296b84b3e9ae7e3ba56c252495bb92c9))

### Testing

- Fix panic in test stream ([#21](https://github.com/aschey/stream-download-rs/pull/21)) - ([90ca324](https://github.com/aschey/stream-download-rs/commit/90ca324247c018c620f21fc83d45b3778117cdd1))
- Use axum as file server ([#31](https://github.com/aschey/stream-download-rs/pull/31)) - ([44ffe20](https://github.com/aschey/stream-download-rs/commit/44ffe20b7153fb71bd3e1fcf7ff478df017ff6a0))

<!-- generated by git-cliff -->
## [0.3.0](https://github.com/aschey/stream-download-rs/compare/v0.2.0..v0.3.0) - 2023-09-05

### Features

- *(storage)* Make tempfile storage optional
- *(storage)* [**breaking**] Make storage configurable

### Miscellaneous Tasks

-  Scrape doc examples
-  Update toolchain
-  Simplify doc links and add feature labels

### Refactor

- *(http)* [**breaking**] Move reqwest module inside the http module

<!-- generated by git-cliff -->
## [0.2.0](https://github.com/aschey/stream-download-rs/compare/v0.1.0..v0.2.0) - 2023-08-17

### Bug Fixes

- *(http)* Return default error if one is not provided

### Features

- *(http)* [**breaking**] Expose stream content type and headers

### Miscellaneous Tasks

-  Update git-cliff commit parsers
-  Check for github actions updates
-  Update ci triggers
-  Update git-cliff changelog format
-  Add dependabot config
-  Add additional rustfmt settings
-  Add conventional commit checks
-  Add git cliff configuration
-  Add release-plz

<!-- generated by git-cliff -->
