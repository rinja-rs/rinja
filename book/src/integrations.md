# Integrations

## Rocket integration

You can use the `rocket` integration with the [rinja_rocket] crate.

In your template definitions, replace `rinja::Template` with
[`rinja_rocket::Template`][rinja_rocket].

See [the example](https://github.com/rinja-rs/rinja/blob/master/rinja_rocket/tests/basic.rs)
from the Rinja test suite for more on how to integrate.

In case a run-time error occurs during templating, a `500 Internal Server
Error` `Status` value will be returned, so that this can be further
handled by your error catcher.

## Actix-web integration

You can use the `actix` integration with the [rinja_actix] crate.

In your template definitions, replace `rinja::Template` with
[`rinja_actix::Template`][rinja_actix].

See [the example](https://github.com/rinja-rs/rinja/blob/master/rinja_actix/tests/basic.rs)
from the Rinja test suite for more on how to integrate.

## Axum integration

You can use the `axum` integration with the [rinja_axum] crate.

In your template definitions, replace `rinja::Template` with
[`rinja_axum::Template`][rinja_axum].

See [the example](https://github.com/rinja-rs/rinja/blob/master/rinja_axum/tests/basic.rs)
from the Rinja test suite for more on how to integrate.

In case of a run-time error occurring during templating, the response will be of the same
signature, with a status code of `500 Internal Server Error`, mime `*/*`, and an empty `Body`.
This preserves the response chain if any custom error handling needs to occur.

## Warp integration

You can use the `warp` integration with the [rinja_warp] crate.

In your template definitions, replace `rinja::Template` with
[`rinja_warp::Template`][rinja_warp].

See [the example](https://github.com/rinja-rs/rinja/blob/master/rinja_warp/tests/warp.rs)
from the Rinja test suite for more on how to integrate.

[rinja_rocket]: https://docs.rs/rinja_rocket
[rinja_actix]: https://docs.rs/rinja_actix
[rinja_axum]: https://docs.rs/rinja_axum
[rinja_warp]: https://docs.rs/rinja_warp
