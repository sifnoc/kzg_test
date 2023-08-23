# Testing KZG commitment for Entries

The Entry refers to an element of the Merkle sum tree that is utilized in Summa. 

We conducted tests by evaluating a commit on the KZG commitment polynomial with multiple entries and then generating a proof with the KZGCircuit. This circuit is developed by Punwai and can be found [here](https://github.com/punwai/halo2-lib/tree/kzg)

## Test
This project includes support for different configurations through feature flags. 

By default, the code will use the `./src/entry_16.csv` file. 

You can compile and run the project using the standard commands:
```bash
> cargo run
```

## Test with large entries
If you want to test with large set $2^{15}$, the `./src/two_assets_entry_2_15.csv` file instead, you can enable the large feature when building or running the project:
```bash
cargo run --features large-entry
```
