## arcanum

minimal, opinionated, correct* operator to sync secrets between Hashicorp Vault and Kubernetes

## What?

a controller that watches SyncedSecret CRDs, which contain an encrypted version of a regular Secret
- If the secret exists in Vault, the secret is pulled from Vault to the Cluster.
- If the secret exists in the cluster and not in Vault, the secret is pushed from the Cluster to Vault.
- If the secret does not exist in the cluster or in Vault, the secret is decrypted from the object itself, and then pushed to Vault.

the controller attempts to gracefully handle Vault being offline (e.x. for bootstrapping)
- If Vault is unreachable and the secret does not exist, it will be created from the encrypted values.
- If Vault is unreachable and the secret does exist, the existing secret will be left alone.

## Why?

existing solutions do not...
- gracefully handle the secret provider being offline
- push existing secrets on the cluster to the secret provider if they don't already exist

If these are not important to you (e.x. fresh cluster on a cloud provider that has a secrets provider builtin), then this is not for you. If you're hosting things yourself (e.x. self-contained bare metal cluster) then this could be helpful.
