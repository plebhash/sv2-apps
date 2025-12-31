use integration_tests_sv2::{
    interceptor::{IgnoreMessage, MessageDirection},
    template_provider::DifficultyLevel,
    *,
};
use stratum_apps::stratum_core::{common_messages_sv2::*, job_declaration_sv2::*, parsers_sv2::{AnyMessage, Mining}};

// Pool propagates block via IPC
#[tokio::test]
async fn pool_propagates_block_with_bitcoin_core_ipc() {
    start_tracing();
    let bitcoin_core = start_bitcoin_core(DifficultyLevel::Low);
    let ipc_socket_path = bitcoin_core.ipc_socket_path().clone();
    let current_block_hash = bitcoin_core.get_best_block_hash().unwrap();
    let (_pool, pool_addr) = start_pool(ipc_config(ipc_socket_path), vec![], vec![]).await;
    let (_translator, tproxy_addr) =
        start_sv2_translator(&[pool_addr], false, vec![], vec![]).await;
    let (_minerd_process, _minerd_addr) = start_minerd(tproxy_addr, None, None, false).await;
    let timeout = tokio::time::Duration::from_secs(60);
    let poll_interval = tokio::time::Duration::from_secs(2);
    let start_time = tokio::time::Instant::now();
    loop {
        tokio::time::sleep(poll_interval).await;
        let new_block_hash = bitcoin_core.get_best_block_hash().unwrap();
        if new_block_hash != current_block_hash {
            return;
        }
        if start_time.elapsed() > timeout {
            panic!(
                "Pool with BitcoinCoreIpc should have propagated a new block within {} seconds",
                timeout.as_secs()
            );
        }
    }
}

// JDC propagates block via IPC (PushSolution blocked to ensure IPC path)
#[tokio::test]
async fn jdc_propagates_block_with_bitcoin_core_ipc() {
    start_tracing();
    let (tp, tp_addr) = start_template_provider(None, DifficultyLevel::Low);
    let ipc_socket_path = tp.ipc_socket_path().clone();
    let current_block_hash = tp.get_best_block_hash().unwrap();
    let (_pool, pool_addr) = start_pool(sv2_tp_config(tp_addr), vec![], vec![]).await;
    let (_jds, jds_addr) = start_jds(tp.rpc_info());
    let ignore_push_solution =
        IgnoreMessage::new(MessageDirection::ToUpstream, MESSAGE_TYPE_PUSH_SOLUTION);
    let (sniffer, sniffer_addr) = start_sniffer(
        "0",
        jds_addr,
        false,
        vec![ignore_push_solution.into()],
        None,
    );
    let (_jdc, jdc_addr) = start_jdc(
        &[(pool_addr, sniffer_addr)],
        ipc_config(ipc_socket_path),
        vec![],
        vec![],
    );
    let (_translator, tproxy_addr) = start_sv2_translator(&[jdc_addr], false, vec![], vec![]).await;
    let (_minerd_process, _minerd_addr) = start_minerd(tproxy_addr, None, None, false).await;
    sniffer
        .wait_for_message_type(MessageDirection::ToUpstream, MESSAGE_TYPE_SETUP_CONNECTION)
        .await;
    sniffer
        .wait_for_message_type(
            MessageDirection::ToDownstream,
            MESSAGE_TYPE_SETUP_CONNECTION_SUCCESS,
        )
        .await;
    sniffer
        .wait_for_message_type(
            MessageDirection::ToUpstream,
            MESSAGE_TYPE_ALLOCATE_MINING_JOB_TOKEN,
        )
        .await;
    sniffer
        .wait_for_message_type(
            MessageDirection::ToDownstream,
            MESSAGE_TYPE_ALLOCATE_MINING_JOB_TOKEN_SUCCESS,
        )
        .await;
    let timeout = tokio::time::Duration::from_secs(60);
    let poll_interval = tokio::time::Duration::from_secs(2);
    let start_time = tokio::time::Instant::now();
    loop {
        tokio::time::sleep(poll_interval).await;
        let new_block_hash = tp.get_best_block_hash().unwrap();
        if new_block_hash != current_block_hash {
            sniffer
                .assert_message_not_present(
                    MessageDirection::ToUpstream,
                    MESSAGE_TYPE_PUSH_SOLUTION,
                )
                .await;
            return;
        }
        if start_time.elapsed() > timeout {
            panic!(
                "JDC with BitcoinCoreIpc should have propagated a new block within {} seconds",
                timeout.as_secs()
            );
        }
    }
}

#[tokio::test]
async fn test_33965() {
    start_tracing();
    let bitcoin_core = start_bitcoin_core(DifficultyLevel::Low);
    bitcoin_core.fund_wallet().unwrap();
    let ipc_socket_path = bitcoin_core.ipc_socket_path().clone();
    let (_pool, pool_addr) = start_pool(ipc_config(ipc_socket_path), vec![], vec![]).await;
    let (sniffer, sniffer_addr) = start_sniffer("0", pool_addr, false, vec![], None);
    let (_translator, tproxy_addr) =
        start_sv2_translator(&[sniffer_addr], false, vec![], vec![]).await;

    sniffer.wait_for_message_type(MessageDirection::ToDownstream, MESSAGE_TYPE_SETUP_CONNECTION_SUCCESS).await;
    let (_minerd_process, _minerd_addr) = start_minerd(tproxy_addr, None, None, false).await;

    loop {
        // create a mempool transaction to trigger a new template with merkle_path.len() > 0
        bitcoin_core.create_mempool_transaction().unwrap();
        let new_extended_mining_job = loop {
            match sniffer.next_message_from_upstream() {
                Some((_, AnyMessage::Mining(Mining::NewExtendedMiningJob(msg)))) => {
                    break msg;
                }
                _ => {
                    // allow other tasks to run
                    tokio::task::yield_now().await;
                    continue;
                }
            }
        };
        println!("new_extended_mining_job: {:?}", new_extended_mining_job);
        let merkle_path = new_extended_mining_job.merkle_path.to_vec();

        // if the merkle path is not empty, the template contains at least one transaction
        if merkle_path.len() > 0 {
            break;
        }
    };
}