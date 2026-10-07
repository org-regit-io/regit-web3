// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Host timers preserve a shared monotonic operation deadline.

#[cfg(feature = "http")]
use std::time::Duration;

#[cfg(feature = "http")]
use crate::error::Error;

#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
pub(crate) use tokio::time::Instant;
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
pub(crate) use web_time::Instant;

#[cfg(feature = "http")]
pub(crate) async fn sleep(duration: Duration) -> Result<(), Error> {
    #[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
    tokio::time::sleep(duration).await;
    #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
    browser::Timer::new(duration)?.wait().await?;
    Ok(())
}

#[cfg(feature = "ton-http")]
pub(crate) async fn sleep_until(deadline: Instant) -> Result<(), Error> {
    #[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
    tokio::time::sleep_until(deadline).await;
    #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
    if let Some(remaining) = deadline.checked_duration_since(Instant::now()) {
        sleep(remaining).await?;
    }
    Ok(())
}

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
pub(crate) use browser::{check_host, timeout_at};

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
mod browser {
    use std::{future::Future, task::Poll, time::Duration};

    use js_sys::{Function, Promise, Reflect};
    use wasm_bindgen::{JsCast, JsValue, closure::Closure, prelude::wasm_bindgen};
    use wasm_bindgen_futures::JsFuture;
    use web_time::Instant;

    use crate::error::Error;

    #[wasm_bindgen]
    extern "C" {
        #[wasm_bindgen(catch, js_name = setTimeout)]
        fn set_timeout(callback: &Function, milliseconds: u32) -> Result<JsValue, JsValue>;
        #[wasm_bindgen(catch, js_name = clearTimeout)]
        fn clear_timeout(identifier: &JsValue) -> Result<(), JsValue>;
    }

    pub(super) struct Timer {
        future: JsFuture,
        identifier: JsValue,
        settle: Function,
        // The JS callback must remain alive until the timer fires or is cancelled.
        _callback: Closure<dyn FnMut()>,
    }

    impl Timer {
        pub(super) fn new(duration: Duration) -> Result<Self, Error> {
            let milliseconds = u32::try_from(duration.as_millis())
                .map_err(|_| Error::Configuration)?
                .saturating_add(u32::from(
                    !duration.subsec_nanos().is_multiple_of(1_000_000),
                ));
            let mut scheduled = None;
            let promise = Promise::new(&mut |resolve, _reject| {
                let settle = resolve.clone();
                let callback = Closure::wrap(Box::new(move || {
                    let _resolved = resolve.call0(&JsValue::UNDEFINED);
                }) as Box<dyn FnMut()>);
                scheduled = Some(
                    set_timeout(callback.as_ref().unchecked_ref(), milliseconds)
                        .map(|identifier| (identifier, callback, settle)),
                );
            });
            let (identifier, callback, settle) = scheduled
                .ok_or(Error::Configuration)?
                .map_err(|_| Error::Configuration)?;
            Ok(Self {
                future: JsFuture::from(promise),
                identifier,
                settle,
                _callback: callback,
            })
        }

        pub(super) async fn wait(&mut self) -> Result<(), Error> {
            (&mut self.future)
                .await
                .map(|_value| ())
                .map_err(|_| Error::Configuration)
        }
    }

    impl Drop for Timer {
        fn drop(&mut self) {
            let _cancelled = clear_timeout(&self.identifier);
            // JsFuture retains its callback/waker state until its Promise
            // settles. Cancelling only the JS timeout would leave that state
            // permanently pending. Resolve on cancellation as well; resolving
            // an already completed Promise is harmless.
            let _settled = self.settle.call0(&JsValue::UNDEFINED);
        }
    }

    pub(crate) fn check_host() -> Result<(), Error> {
        let global = js_sys::global();
        for name in ["setTimeout", "clearTimeout"] {
            let value = Reflect::get(&global, &JsValue::from_str(name))
                .map_err(|_| Error::Configuration)?;
            if !value.is_function() {
                return Err(Error::Configuration);
            }
        }
        let performance = Reflect::get(&global, &JsValue::from_str("performance"))
            .map_err(|_| Error::Configuration)?;
        let now = Reflect::get(&performance, &JsValue::from_str("now"))
            .map_err(|_| Error::Configuration)?;
        if !now.is_function() {
            return Err(Error::Configuration);
        }
        Ok(())
    }

    pub(crate) async fn timeout_at<T, F>(deadline: Instant, operation: F) -> Result<T, Error>
    where
        F: Future<Output = Result<T, Error>>,
    {
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .filter(|duration| !duration.is_zero())
            .ok_or(Error::Timeout)?;
        let mut timer = Timer::new(remaining)?;
        let mut timer_future = Box::pin(timer.wait());
        let mut operation = Box::pin(operation);
        std::future::poll_fn(|context| {
            // Check the clock first so a completed operation cannot outrun a
            // delayed/throttled host timer after the absolute deadline.
            if Instant::now() >= deadline {
                return Poll::Ready(Err(Error::Timeout));
            }
            if let Poll::Ready(result) = timer_future.as_mut().poll(context) {
                return Poll::Ready(result.and(Err(Error::Timeout)));
            }
            operation.as_mut().poll(context)
        })
        .await
    }
}
