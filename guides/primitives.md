# Core primitives

[Overview](../README.md) · [Primitives](primitives.md) · [Catalogue](catalogue.md) · [Contracts](contracts.md) · [Features](features.md) · [Qualification](qualification.md) · [Development](development.md)

Exact arithmetic, explicit asset mappings and bounded observation collections are available with empty default features. Family-specific identities and observations select their own pure capability feature; none of these operations fetch data or require a runtime.

## Exact arithmetic and conversion

| API | Contract |
| --- | --- |
| `ExactDecimal::{checked_add, checked_sub, checked_mul}` | Exact signed arithmetic; rejects operations or results outside decimal resource bounds |
| `ExactDecimal::quantize` | Explicit scale and `RoundingMode`; scale `2` means hundredths, `0` integers and `-2` hundreds |
| `Amount::{checked_add, checked_sub}` | Checked unsigned base units; both operands require known, equal precision |
| `Amount::checked_mul` | Checked multiplication by a dimensionless `U256` integer scalar; preserves known precision |
| `Amount::{to_exact_decimal, from_exact_decimal}` | Exact conversion between known base-unit precision and nonnegative decimals; fractional base units fail |
| `Amount::{from_exact_decimal_rounded, checked_rescale}` | Explicit precision and rounding; negative decimals cannot become unsigned amounts |

`ArithmeticError` reports fixed categories without operands. Unknown precision remains representable in `Amount`, but arithmetic and conversion require declared precision. Matching precision alone does not establish asset identity. Decimal results are normalized numeric values; quantization does not retain display padding or declare an asset's decimals.

| Rounding mode | Rule |
| --- | --- |
| `RejectInexact` | Fail when nonzero digits would be discarded |
| `TowardZero`, `AwayFromZero` | Discard toward zero, or away whenever discarded digits are nonzero |
| `Floor`, `Ceiling` | Round toward negative or positive infinity |
| `HalfUp`, `HalfDown`, `HalfEven` | Nearest value; ties go away from zero, toward zero, or to an even retained digit |

This example needs no optional features. The quantity and unit price are synthetic numbers. The caller supplies asset identity, quote currency, price source/time and rounding policy alongside its original records; multiplication performs only the numeric operation.

```rust
use regit_web3::domain::{Amount, ExactDecimal, RoundingMode};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let quantity = Amount::from_decimal("2500000", Some(6))?;
    let unit_price = ExactDecimal::parse("3.333")?;
    let product = quantity.to_exact_decimal()?.checked_mul(&unit_price)?;
    assert_eq!(product.canonical(), "8.3325");

    let rounded = product.quantize(2, RoundingMode::HalfEven)?;
    let quote_units = Amount::from_exact_decimal(&rounded, 2)?;
    assert_eq!(quote_units.formatted().as_deref(), Some("8.33"));
    Ok(())
}
```

## Explicit asset-to-market bindings

`domain::asset_binding` records a caller-declared relationship using existing family identity types and core `domain::market::Identifier`. A listing combines an exact, case-sensitive provider namespace and listing ID. Mapping `Source` and `Timestamp` describe the mapping assertion, separately from market observation attribution and retrieval time.

| API | Contract |
| --- | --- |
| `MarketListing` | Exact validated provider/listing identifiers |
| `AssetMarketBinding<A>` | Immutable full asset key, listing, source and recorded time |
| `AssetMarketRegistry<A>` | Explicit capacity 1–1,024; duplicate and conflicting asset/provider pairs fail; exact lookup |

Use stable, fully qualified technical keys: native `Asset::identity()`, an ERC-20 `(ChainId, Address)` pair, or Solana `TokenIdentity` with full genesis, mint and token program. Generic keys preserve their own equality, validation and serialization contracts. Display symbols, aliases and precision metadata do not identify an asset; metadata-bearing wrappers may include aliases in equality, so choose the technical key explicitly.

Enable the pure `solana` feature for this example (`features = ["solana"]`, `default-features = false`). All bytes, labels and timestamps below are synthetic. They demonstrate a local mapping without asserting that a token or listing exists.

```rust
use regit_web3::domain::{
    Source, Timestamp,
    asset_binding::{AssetMarketBinding, AssetMarketRegistry, MarketListing},
    market::Identifier,
    solana::{Hash, Pubkey, TokenIdentity},
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let key = TokenIdentity::new(
        Hash::from_bytes([1; 32]),
        Pubkey::from_bytes([2; 32]),
        Pubkey::from_bytes([3; 32]),
    );
    let provider = Identifier::parse("example-provider")?;
    let binding = AssetMarketBinding::new(
        key,
        MarketListing::new(provider.clone(), Identifier::parse("synthetic-listing")?),
        Source::new("example-registry", "configured-mapping", "v1")?,
        Timestamp::from_unix_seconds(100),
    );
    let registry = AssetMarketRegistry::new(4, vec![binding])?;
    let found = registry.lookup(&key, &provider).expect("configured mapping");
    assert_eq!(found.listing().listing_id().as_str(), "synthetic-listing");
    Ok(())
}
```

Lookup neither falls back to another provider nor guesses from symbols. Several technical keys may explicitly share a listing; this does not verify economic equivalence. Bindings do not resolve token metadata, infer decimals, authenticate a source or choose a price.

## Bounded observation collections

`domain::collections::ObservationCollection<K, O>` retains ordered, unique caller-selected keys and `Result<O, Error>` outcomes. Successful observations remain unchanged, including their original family network, evaluation anchor, source and retrieval facts. Failures retain their exact key and fixed error category.

| API | Contract |
| --- | --- |
| `CollectionLimit` | Explicit maximum 1–1,024; no unlimited or implicit default |
| `ObservationItem<K, O>` | Exact key and typed success/failure; caller establishes the key-to-observation relationship |
| `ObservationCollection<K, O>` | Rejects excess items and duplicate keys, including failed-item keys; preserves input order and exact lookup |
| `success_count`, `failure_count`, `all_succeeded` | Describe supplied outcomes only; `all_succeeded()` returns `true` for an empty collection |

This example also requires only the pure `solana` feature. Its network, account, balance, slot and times are synthetic caller-supplied records; construction makes no remote inclusion or balance claim.

```rust
use regit_web3::{
    domain::{
        Source, Timestamp,
        collections::{CollectionLimit, ObservationCollection, ObservationItem},
        solana::{Commitment, Context, Hash, NativeBalance, Network, Observation,
                 Operation, Pubkey, ReadOptions},
    },
    error::Error,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let genesis = Hash::from_bytes([1; 32]);
    let address = Pubkey::from_bytes([2; 32]);
    let network = Network::new(genesis, "synthetic-network")?;
    let context = Context::new(
        Operation::NativeBalance, network.clone(),
        ReadOptions::new(Commitment::Confirmed, Some(40)), 42,
        Source::new("example-source", "balance", "v1")?,
        Timestamp::from_unix_seconds(100),
    )?;
    let observation = Observation::native_balance(
        NativeBalance::new(network, address, 1_000_000_000), context,
    )?;
    let collection = ObservationCollection::new(CollectionLimit::new(4)?, vec![
        ObservationItem::new((genesis, address), Ok(observation)),
        ObservationItem::new((genesis, Pubkey::from_bytes([3; 32])), Err(Error::Timeout)),
    ])?;
    assert_eq!((collection.success_count(), collection.failure_count()), (1, 1));
    let item = collection.get(&(genesis, address)).expect("supplied item");
    assert_eq!(item.observation().expect("successful item").context().slot(), 42);
    Ok(())
}
```

Collections do not fetch omitted items, establish remote completeness or impose a common snapshot across sources or families. Callers can use their own tagged key/value types for multiple families while retaining each family's context. Constructors and deserialization enforce the same limits and uniqueness. Deserialization rejects items beyond the active decoding limit before decoding their key or outcome. When `limit` precedes `items`, that limit applies during decoding; otherwise the absolute ceiling applies and the configured limit is checked afterward. Item limits bound count, while each key/value type owns its own byte and resource bounds.
