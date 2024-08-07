# Changelog

All notable changes to this project will be documented in this file.

## [0.7.1](https://github.com/aschey/stream-download-rs/compare/0.7.0..0.7.1) - 2024-08-02

### Dependencies

- *(deps)* Update opendal requirement from 0.47 to 0.48 ([#77](https://github.com/aschey/stream-download-rs/pull/77)) - ([bb35f63](https://github.com/aschey/stream-download-rs/commit/bb35f63bec85a1b846897804f0220503be04b554))
- *(deps)* Update testcontainers-modules requirement from 0.8.0 to 0.9.0 ([#78](https://github.com/aschey/stream-download-rs/pull/78)) - ([c30d9a1](https://github.com/aschey/stream-download-rs/commit/c30d9a1406eac1031764080dea4935cb3388753d))

<!-- generated by git-cliff -->
## [0.7.0](https://github.com/aschey/stream-download-rs/compare/0.6.0..0.7.0) - 2024-07-28

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
## [0.6.0](https://github.com/aschey/stream-download-rs/compare/0.5.2..0.6.0) - 2024-07-06

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
## [0.5.2](https://github.com/aschey/stream-download-rs/compare/0.5.1..0.5.2) - 2024-04-04

### Bug Fixes

- Handle error statuses from http requests ([#50](https://github.com/aschey/stream-download-rs/pull/50)) - ([a4ec987](https://github.com/aschey/stream-download-rs/commit/a4ec98739d114df2e7c37cd7679e02f61fdadb1f))

<!-- generated by git-cliff -->
## [0.5.1](https://github.com/aschey/stream-download-rs/compare/0.5.0..0.5.1) - 2024-03-31

### Bug Fixes

- Only log appropriate seek errors ([#49](https://github.com/aschey/stream-download-rs/pull/49)) - ([b6a900c](https://github.com/aschey/stream-download-rs/commit/b6a900cde093cab486fd886e45a3ca58d9862645))

### Features

- Use native async traits ([#45](https://github.com/aschey/stream-download-rs/pull/45)) - ([3c61e78](https://github.com/aschey/stream-download-rs/commit/3c61e78e0c475688f085d504eaf1e1533df835c9))
- Make seek buffer size configurable ([#48](https://github.com/aschey/stream-download-rs/pull/48)) - ([42783d6](https://github.com/aschey/stream-download-rs/commit/42783d6294f6d6419e04f65d2aac68090412e685))

### Miscellaneous Tasks

- Update reqwest to 0.12 ([#46](https://github.com/aschey/stream-download-rs/pull/46)) - ([ed75e88](https://github.com/aschey/stream-download-rs/commit/ed75e880a89b89bc780fdf1894c1bec228ccb686))

<!-- generated by git-cliff -->
## [0.5.0](https://github.com/aschey/stream-download-rs/compare/0.4.2..0.5.0) - 2024-03-20

### Features

- *(storage)* Make tempfile more configurable ([#42](https://github.com/aschey/stream-download-rs/pull/42)) - ([c71de8b](https://github.com/aschey/stream-download-rs/commit/c71de8baeceafa0a43ec5b953741a24f1ac73888))

### Miscellaneous Tasks

- Update dev-dependencies - ([d42514e](https://github.com/aschey/stream-download-rs/commit/d42514e11a70ebd4f39dee90e2fb59c1d1b3e22b))

<!-- generated by git-cliff -->
## [0.4.2](https://github.com/aschey/stream-download-rs/compare/0.4.1..0.4.2) - 2024-02-10

### Bug Fixes

- Use proper buffer size with BoundedStorageProvider when content length is known ([#37](https://github.com/aschey/stream-download-rs/pull/37)) - ([66182d8](https://github.com/aschey/stream-download-rs/commit/66182d8c5ee02f3c889cb1076f9f717838a23ae6))
- Ensure content length fits within the requested buffer ([#39](https://github.com/aschey/stream-download-rs/pull/39)) - ([6a15e94](https://github.com/aschey/stream-download-rs/commit/6a15e94ace9d26e489adf676c7d2ad3f919f9ed6))

<!-- generated by git-cliff -->
## [0.4.1](https://github.com/aschey/stream-download-rs/compare/0.4.0..0.4.1) - 2024-02-08

### Bug Fixes

- Fix release build ([#34](https://github.com/aschey/stream-download-rs/pull/34)) - ([8cc3163](https://github.com/aschey/stream-download-rs/commit/8cc31632f9783fa6fbe783b2ec1b7adda8e23b93))

<!-- generated by git-cliff -->
## [0.4.0] - 2023-12-03

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
## [0.3.0] - 2023-09-05

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
## [0.2.0] - 2023-08-17

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
