// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Regit

//! Generic EVM reads using a caller-owned, runtime-independent implementation.

#![cfg(test)]
#![cfg(feature = "evm")]

use std::{
    future::{Future, ready},
    task::{Context, Poll, Waker},
};

use regit_web3::{
    chains::evm::NativeBalanceReader,
    domain::{
        Address, Amount, Asset, Balance, BlockContext, BlockHash, BlockSelector, ChainId, Finality,
        NetworkId, Observation, ObservationContext, Source, Timestamp,
    },
    error::{Error, ValidationError},
    future::MaybeSend,
};

struct CallerReader {
    network: NetworkId,
    default_selector: BlockSelector,
}

impl NativeBalanceReader for CallerReader {
    fn get_native_balance(
        &self,
        address: Address,
        selector: Option<BlockSelector>,
    ) -> impl Future<Output = Result<Observation<Balance>, Error>> + MaybeSend {
        let observation = (|| {
            let value = Balance::new(
                address,
                Asset::native(self.network.clone(), 6, Some("NATIVE".to_owned()))?,
                Amount::from_decimal("9007199254740993", Some(6))?,
            )?;
            let context = ObservationContext::new(
                self.network.clone(),
                selector.unwrap_or(self.default_selector),
                BlockContext::new(
                    42,
                    BlockHash::from_bytes([7; 32]),
                    Timestamp::from_unix_seconds(100),
                ),
                Source::new("caller-fixture", "native_balance", "1")?,
                Timestamp::from_unix_seconds(105),
            )?;
            Observation::native_balance(value, context)
        })();
        ready(observation)
    }
}

fn generic_read<R: NativeBalanceReader>(
    reader: &R,
    address: Address,
    selector: Option<BlockSelector>,
) -> impl Future<Output = Result<Observation<Balance>, Error>> + MaybeSend {
    reader.get_native_balance(address, selector)
}

fn poll_ready<F: Future + MaybeSend>(future: F) -> F::Output {
    #[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
    {
        fn assert_send<T: Send>(_: &T) {}
        assert_send(&future);
    }
    let mut future = std::pin::pin!(future);
    let mut context = Context::from_waker(Waker::noop());
    match future.as_mut().poll(&mut context) {
        Poll::Ready(value) => value,
        Poll::Pending => unreachable!("caller fixture must be immediately ready"),
    }
}

#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn generic_callers_use_custom_readers_with_exact_values_and_explicit_defaults() -> Result<(), Error>
{
    let reader = CallerReader {
        network: NetworkId::new(ChainId::from(1), "caller-network")?,
        default_selector: BlockSelector::Safe,
    };
    let address = Address::from_bytes([3; 20]);
    let observed = poll_ready(generic_read(&reader, address, None))?;
    assert_eq!(observed.value().address(), &address);
    assert_eq!(
        observed.value().amount().raw().to_string(),
        "9007199254740993"
    );
    assert_eq!(
        observed.value().amount().formatted(),
        Some("9007199254.740993".to_owned())
    );
    assert_eq!(
        observed.context().requested_selector(),
        &BlockSelector::Safe
    );
    assert_eq!(observed.context().block().number(), 42);
    assert_eq!(observed.context().source().provider_id(), "caller-fixture");
    assert_eq!(observed.context().retrieved_at().unix_seconds(), 105);
    assert_eq!(observed.context().finality(), Finality::Unknown);
    assert_eq!(observed.context().confirmations(), None);
    Ok(())
}

#[cfg_attr(
    all(target_arch = "wasm32", target_os = "unknown"),
    wasm_bindgen_test::wasm_bindgen_test
)]
#[cfg_attr(not(all(target_arch = "wasm32", target_os = "unknown")), test)]
fn generic_capability_preserves_explicit_selector_and_typed_failures() -> Result<(), Error> {
    let reader = CallerReader {
        network: NetworkId::new(ChainId::from(1), "caller-network")?,
        default_selector: BlockSelector::Latest,
    };
    let address = Address::from_bytes([3; 20]);
    let selector = BlockSelector::Hash(BlockHash::from_bytes([7; 32]));
    let observed = poll_ready(generic_read(&reader, address, Some(selector)))?;
    assert_eq!(observed.context().requested_selector(), &selector);
    assert!(matches!(
        poll_ready(generic_read(
            &reader,
            address,
            Some(BlockSelector::Number(41))
        )),
        Err(Error::Validation(ValidationError::BlockMismatch))
    ));
    Ok(())
}

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
#[wasm_bindgen_test::wasm_bindgen_test]
async fn generic_capability_executes_a_local_browser_future() -> Result<(), Error> {
    use std::rc::Rc;

    struct LocalReader(Rc<CallerReader>);
    impl NativeBalanceReader for LocalReader {
        fn get_native_balance(
            &self,
            address: Address,
            selector: Option<BlockSelector>,
        ) -> impl Future<Output = Result<Observation<Balance>, Error>> + MaybeSend {
            let reader = self.0.clone();
            async move { reader.get_native_balance(address, selector).await }
        }
    }

    fn local_shared_client<T: regit_web3::future::MaybeSync>(_: &T) {}
    let reader = LocalReader(Rc::new(CallerReader {
        network: NetworkId::new(ChainId::from(1), "browser-local")?,
        default_selector: BlockSelector::Safe,
    }));
    local_shared_client(&reader);
    let address = Address::from_bytes([3; 20]);
    let observed = generic_read(&reader, address, None).await?;
    assert_eq!(observed.value().address(), &address);
    assert_eq!(
        observed.value().amount().raw().to_string(),
        "9007199254740993"
    );
    assert_eq!(
        observed.context().requested_selector(),
        &BlockSelector::Safe
    );
    Ok(())
}
