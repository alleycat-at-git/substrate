## Task

### Description

Create a fork of Substrate Parity from: https://github.com/paritytech/substrate

Modify this fork so that when transactions are sent to the blockchain, they are first sorted into two queues, where transactions sent from addresses that hash (using any string to integer hash function) into an odd number are assigned to sorting queue 1, and transactions sent from addresses that hash into an even number are assigned to sorting queue 2.

Then when the consensus algorithm is building blocks, it will pop the transactions alternately from sorting queue 1 and sorting queue 2 for each block. So one block will contain only transactions of from queue 1, and the next block will have only transactions of queue 2, and ad infinitum.

## Solution

### Description

In my solution I focused on two things:

1) Minimize the size of the solution (i.e. LOC used)
2) Minimize altering the core `Substrate` code, i.e. try to implement the solution in runtime logic

I dug into several apis provided to runtime and found that `TaggedTransactionQueue` should do the trick. It allows to put required tags on the transaction and postpone the execution of transaction until some other transaction provides this tag.

For our problem, we check if transaction should be included in the current block or the next block. If we need to postpone it for the next block, we put a predefined tag (I called it `QUEUE_TAG`) on it.

Now the problem is how to provide the tag in the next block. A naive and easy approach would be to provide it with the transactions the next block. But the problem is there could be no transactions in the next block. In this case, our pending transactions could be postponed for a long time.

To solve this problem, I had to patch the transaction pool in the core and forcefully provide this tag to execute postponed transactions. Again this could be done by brutally moving all deferred transactions to block but I made it in the least intrusive way, so that tags functionality still work for other tags.

### Hash function used

Last bit of the public key.

### Deliverable

I made it as a separate PR into `1.0.0rc2.1`. I had to start from `1.0.0rc2.1` because `substrate-ui` is not up-to-date with `1.0.0` or `master` (`substrate` uses metadata v7 but `substrate-ui` only knows how to parse metadata v4 as of now).

The problem with `1.0.0rc2.1` though is that it doesn't work out of the box with current nightly, so I had to bump some dependencies. I put all this non-relevant code in the first commit of the PR.


### Build and run

```bash
echo "Building WASM"
cd ./node/runtime/wasm
./build.sh
cd ../../../
echo "Purging"
cargo run -- purge-chain --chain dev
echo "Starting node"
cargo run -- --dev
```
