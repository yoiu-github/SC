#[cfg(test)]
mod tests {
    use crate::contract::{handle, init, query};
    use crate::msg::{ContractStatus, HandleMsg, InitMsg, Mint, QueryAnswer, QueryMsg};
    use crate::token::{Extension, Metadata};
    use cosmwasm_std::{from_binary, testing::*, Api, HandleResponse, Querier, StdError, Storage};
    use cosmwasm_std::{Extern, HumanAddr, StdResult};
    use std::any::Any;

    fn init_helper() -> Extern<MockStorage, MockApi, MockQuerier> {
        let mut deps = mock_dependencies(20, &[]);
        let env = mock_env("instantiator", &[]);

        let init_msg = InitMsg {
            name: "sec721".to_string(),
            symbol: "S721".to_string(),
            admin: Some(HumanAddr("admin".to_string())),
            entropy: "We're going to need a bigger boat".to_string(),
            royalty_info: None,
            config: None,
            post_init_callback: None,
        };

        init(&mut deps, env, init_msg).unwrap();
        deps
    }

    fn extract_log(resp: StdResult<HandleResponse>) -> String {
        match resp {
            Ok(response) => response.log[0].value.clone(),
            Err(_err) => "These are not the logs you are looking for".to_string(),
        }
    }

    fn extract_error_msg<T: Any>(error: StdResult<T>) -> String {
        match error {
            Ok(_response) => panic!("Expected error, but had Ok response"),
            Err(err) => match err {
                StdError::GenericErr { msg, .. } => msg,
                _ => panic!("Unexpected error result {:?}", err),
            },
        }
    }

    fn get_tier<S: Storage, A: Api, Q: Querier>(deps: &Extern<S, A, Q>, token_id: &str) -> u8 {
        let msg = QueryMsg::TierOf {
            token_id: token_id.to_string(),
        };
        let query_result = query(deps, msg).unwrap();
        let query_answer: QueryAnswer = from_binary(&query_result).unwrap();
        match query_answer {
            QueryAnswer::TierOf { tier } => tier,
            _ => panic!("Unexpected answer {:?}", query_answer),
        }
    }

    #[test]
    fn test_set_tier() {
        let mut deps = init_helper();
        let token_id = "MyNFT";
        let handle_msg = HandleMsg::MintNft {
            token_id: Some(token_id.to_string()),
            owner: Some(HumanAddr("alice".to_string())),
            public_metadata: Some(Metadata {
                token_uri: None,
                extension: Some(Extension {
                    name: Some(token_id.to_string()),
                    description: None,
                    image: Some("uri".to_string()),
                    ..Extension::default()
                }),
            }),
            private_metadata: None,
            royalty_info: None,
            serial_number: None,
            transferable: None,
            memo: None,
            padding: None,
            tier: None,
        };

        let handle_result = handle(&mut deps, mock_env("admin", &[]), handle_msg);
        let minted = extract_log(handle_result);
        assert!(minted.contains(token_id));
        assert_eq!(0, get_tier(&deps, token_id));

        let handle_msg = HandleMsg::SetContractStatus {
            level: ContractStatus::Normal,
            padding: None,
        };
        handle(&mut deps, mock_env("admin", &[]), handle_msg).unwrap();

        // test tier out of range
        let handle_msg = HandleMsg::SetTier {
            token_id: token_id.to_string(),
            tier: 5,
        };

        let handle_result = handle(&mut deps, mock_env("admin", &[]), handle_msg);
        let error = extract_error_msg(handle_result);
        assert!(error.contains("The tier is out of range"));

        // test non-admin attempt
        let handle_msg = HandleMsg::SetTier {
            token_id: token_id.to_string(),
            tier: 3,
        };

        let handle_result = handle(&mut deps, mock_env("alice", &[]), handle_msg);
        let error = extract_error_msg(handle_result);
        assert!(error.contains("Only designated minters are allowed to change tier"));

        let handle_msg = HandleMsg::SetTier {
            token_id: token_id.to_string(),
            tier: 4,
        };
        let handle_result = handle(&mut deps, mock_env("admin", &[]), handle_msg);

        assert!(
            handle_result.is_ok(),
            "Set tier failed: {}",
            handle_result.err().unwrap()
        );
        assert_eq!(4, get_tier(&deps, token_id));
    }

    #[test]
    fn test_mint_with_tier() {
        let mut deps = init_helper();

        let metadata = Metadata {
            token_uri: None,
            extension: Some(Extension {
                name: Some("MyNFT".to_string()),
                description: None,
                image: Some("uri".to_string()),
                ..Extension::default()
            }),
        };

        // test tier out of range
        let handle_msg = HandleMsg::MintNft {
            token_id: Some("MyNft".to_string()),
            owner: Some(HumanAddr("alice".to_string())),
            public_metadata: Some(metadata.clone()),
            private_metadata: None,
            royalty_info: None,
            serial_number: None,
            transferable: None,
            memo: None,
            padding: None,
            tier: Some(5),
        };

        let handle_result = handle(&mut deps, mock_env("admin", &[]), handle_msg);
        let error = extract_error_msg(handle_result);
        assert!(error.contains("The tier is out of range"));

        for i in 0..5 {
            let token_id = format!("MyNFT_{}", i);
            let handle_msg = HandleMsg::MintNft {
                token_id: Some(token_id.clone()),
                owner: Some(HumanAddr("alice".to_string())),
                public_metadata: Some(metadata.clone()),
                private_metadata: None,
                royalty_info: None,
                serial_number: None,
                transferable: None,
                memo: None,
                padding: None,
                tier: Some(i),
            };

            let handle_result = handle(&mut deps, mock_env("admin", &[]), handle_msg);
            let minted = extract_log(handle_result);
            assert!(minted.contains(&token_id));
            assert_eq!(i, get_tier(&deps, &token_id));
        }
    }

    #[test]
    fn test_batch_mint_with_tier() {
        let mut deps = init_helper();

        let admin = HumanAddr("admin".to_string());
        let alice = HumanAddr("alice".to_string());
        let bob = HumanAddr("bob".to_string());

        let handle_msg = HandleMsg::AddMinters {
            minters: vec![bob.clone()],
            padding: None,
        };
        handle(&mut deps, mock_env(admin.clone(), &[]), handle_msg).unwrap();

        let public_metadata = Metadata {
            token_uri: None,
            extension: Some(Extension {
                name: Some("NFT1".to_string()),
                description: Some("pub".to_string()),
                image: Some("uri1".to_string()),
                ..Extension::default()
            }),
        };

        let private_metadata = Metadata {
            token_uri: None,
            extension: Some(Extension {
                name: Some("NFT2".to_string()),
                description: Some("priv".to_string()),
                image: Some("uri2".to_string()),
                ..Extension::default()
            }),
        };

        let mut mints = vec![
            Mint {
                token_id: Some("NFT3".to_string()),
                owner: Some(alice.clone()),
                public_metadata: Some(public_metadata),
                private_metadata: None,
                royalty_info: None,
                serial_number: None,
                transferable: None,
                memo: None,
                tier: Some(3),
            },
            Mint {
                token_id: Some("WRONGNFT".to_string()),
                owner: None,
                public_metadata: None,
                private_metadata: Some(private_metadata),
                royalty_info: None,
                serial_number: None,
                transferable: None,
                memo: None,
                tier: Some(5),
            },
            Mint {
                token_id: Some("NFT1".to_string()),
                owner: Some(bob.clone()),
                public_metadata: None,
                private_metadata: None,
                royalty_info: None,
                transferable: None,
                serial_number: None,
                memo: None,
                tier: Some(1),
            },
            Mint {
                token_id: Some("NFT0".to_string()),
                owner: Some(admin.clone()),
                public_metadata: None,
                private_metadata: None,
                royalty_info: None,
                transferable: None,
                serial_number: None,
                memo: None,
                tier: None,
            },
            Mint {
                token_id: Some("NFT4".to_string()),
                owner: Some(alice),
                public_metadata: None,
                private_metadata: None,
                royalty_info: None,
                transferable: None,
                serial_number: None,
                memo: Some("has id 4".to_string()),
                tier: Some(4),
            },
            Mint {
                token_id: Some("NFT2".to_string()),
                owner: Some(admin.clone()),
                public_metadata: None,
                private_metadata: None,
                royalty_info: None,
                transferable: None,
                serial_number: None,
                memo: Some("has id 3".to_string()),
                tier: Some(2),
            },
        ];

        let handle_msg = HandleMsg::BatchMintNft {
            mints: mints.clone(),
            padding: None,
        };

        let handle_result = handle(&mut deps, mock_env("admin", &[]), handle_msg);
        let error = extract_error_msg(handle_result);
        assert!(error.contains("The tier is out of range"));

        // sanity check
        // remove wrong mint
        mints.remove(1);
        let mut deps = init_helper();

        let handle_msg = HandleMsg::AddMinters {
            minters: vec![bob.clone()],
            padding: None,
        };
        handle(&mut deps, mock_env(admin, &[]), handle_msg).unwrap();

        let handle_msg = HandleMsg::BatchMintNft {
            mints,
            padding: None,
        };
        handle(&mut deps, mock_env(bob, &[]), handle_msg).unwrap();

        for i in 0..5 {
            let token_id = format!("NFT{}", i);
            assert_eq!(i, get_tier(&deps, &token_id));
        }
    }
}
