# Testing KZG commitment for Entries

The Entry refers to an element of the Merkle sum tree that is utilized in Summa. 

We conducted tests by evaluating a commit on the KZG commitment polynomial with multiple entries and then generating a proof with the KZGCircuit. This circuit is developed by Punwai and can be found [here](https://github.com/punwai/halo2-lib/tree/kzg)
