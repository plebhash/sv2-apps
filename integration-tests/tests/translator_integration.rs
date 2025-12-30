// This file contains integration tests for the `TranslatorSv2` module.
use integration_tests_sv2::{
    interceptor::{MessageDirection, ReplaceMessage},
    template_provider::DifficultyLevel,
    *,
};
use stratum_apps::stratum_core::mining_sv2::*;

use stratum_apps::stratum_core::{
    common_messages_sv2::{
        SetupConnectionError, MESSAGE_TYPE_SETUP_CONNECTION, MESSAGE_TYPE_SETUP_CONNECTION_ERROR,
        MESSAGE_TYPE_SETUP_CONNECTION_SUCCESS,
    },
    mining_sv2::{
        OpenMiningChannelError, MESSAGE_TYPE_OPEN_EXTENDED_MINING_CHANNEL,
        MESSAGE_TYPE_OPEN_EXTENDED_MINING_CHANNEL_SUCCESS,
    },
    parsers_sv2::{self, AnyMessage},
};

// This test runs an sv2 translator between an sv1 mining device and a pool. the connection between
// the translator and the pool is intercepted by a sniffer. The test checks if the translator and
// the pool exchange the correct messages upon connection. And that the miner is able to submit
// shares.
#[tokio::test]
async fn translate_sv1_to_sv2_successfully() {
    start_tracing();
    let (_tp, tp_addr) = start_template_provider(None, DifficultyLevel::Low);
    let (_pool, pool_addr) = start_pool(sv2_tp_config(tp_addr), vec![], vec![]).await;
    let (pool_translator_sniffer, pool_translator_sniffer_addr) =
        start_sniffer("0", pool_addr, false, vec![], None);
    let (_, tproxy_addr) =
        start_sv2_translator(&[pool_translator_sniffer_addr], false, vec![], vec![], None).await;
    let (_minerd_process, _minerd_addr) = start_minerd(tproxy_addr, None, None, false).await;
    pool_translator_sniffer
        .wait_for_message_type(MessageDirection::ToUpstream, MESSAGE_TYPE_SETUP_CONNECTION)
        .await;
    pool_translator_sniffer
        .wait_for_message_type(
            MessageDirection::ToDownstream,
            MESSAGE_TYPE_SETUP_CONNECTION_SUCCESS,
        )
        .await;
    pool_translator_sniffer
        .wait_for_message_type(
            MessageDirection::ToUpstream,
            MESSAGE_TYPE_OPEN_EXTENDED_MINING_CHANNEL,
        )
        .await;
    pool_translator_sniffer
        .wait_for_message_type(
            MessageDirection::ToDownstream,
            MESSAGE_TYPE_OPEN_EXTENDED_MINING_CHANNEL_SUCCESS,
        )
        .await;
    pool_translator_sniffer
        .wait_for_message_type(
            MessageDirection::ToDownstream,
            MESSAGE_TYPE_NEW_EXTENDED_MINING_JOB,
        )
        .await;
    pool_translator_sniffer
        .wait_for_message_type(
            MessageDirection::ToUpstream,
            MESSAGE_TYPE_SUBMIT_SHARES_EXTENDED,
        )
        .await;
}

// Demonstrates the scenario where TProxy falls back to the secondary pool
// after the primary pool returns a `SetupConnection.Error`.
#[tokio::test]
async fn test_translator_fallback_on_setup_connection_error() {
    start_tracing();
    let (_tp, tp_addr) = start_template_provider(None, DifficultyLevel::Low);
    let (_pool_1, pool_addr_1) = start_pool(sv2_tp_config(tp_addr), vec![], vec![]).await;
    let (_pool_2, pool_addr_2) = start_pool(sv2_tp_config(tp_addr), vec![], vec![]).await;

    let random_error_code = "Something went wrong".to_string();

    let setup_connection_success_replace = ReplaceMessage::new(
        MessageDirection::ToDownstream,
        MESSAGE_TYPE_SETUP_CONNECTION_SUCCESS,
        AnyMessage::Common(parsers_sv2::CommonMessages::SetupConnectionError(
            SetupConnectionError {
                flags: 0,
                error_code: random_error_code.try_into().unwrap(),
            },
        )),
    );

    let (pool_translator_sniffer_1, pool_translator_sniffer_addr_1) = start_sniffer(
        "A",
        pool_addr_1,
        false,
        vec![setup_connection_success_replace.into()],
        None,
    );

    let (pool_translator_sniffer_2, pool_translator_sniffer_addr_2) =
        start_sniffer("B", pool_addr_2, false, vec![], None);

    let (_, tproxy_addr) = start_sv2_translator(
        &[
            pool_translator_sniffer_addr_1,
            pool_translator_sniffer_addr_2,
        ],
        false,
        vec![],
        vec![],
        None,
    )
    .await;

    let (_minerd_process, _minerd_addr) = start_minerd(tproxy_addr, None, None, false).await;

    pool_translator_sniffer_1
        .wait_for_message_type(MessageDirection::ToUpstream, MESSAGE_TYPE_SETUP_CONNECTION)
        .await;
    pool_translator_sniffer_1
        .wait_for_message_type(
            MessageDirection::ToDownstream,
            MESSAGE_TYPE_SETUP_CONNECTION_ERROR,
        )
        .await;

    pool_translator_sniffer_2
        .wait_for_message_type(MessageDirection::ToUpstream, MESSAGE_TYPE_SETUP_CONNECTION)
        .await;

    pool_translator_sniffer_2
        .wait_for_message_type(
            MessageDirection::ToDownstream,
            MESSAGE_TYPE_SETUP_CONNECTION_SUCCESS,
        )
        .await;

    pool_translator_sniffer_2
        .wait_for_message_type(
            MessageDirection::ToUpstream,
            MESSAGE_TYPE_OPEN_EXTENDED_MINING_CHANNEL,
        )
        .await;
    pool_translator_sniffer_2
        .wait_for_message_type(
            MessageDirection::ToDownstream,
            MESSAGE_TYPE_OPEN_EXTENDED_MINING_CHANNEL_SUCCESS,
        )
        .await;
}

// Demonstrates the scenario where the primary pool returns an `OpenMiningChannel.Error`,
// causing TProxy to fall back to the secondary pool.
#[tokio::test]
async fn test_translator_fallback_on_open_mining_message_error() {
    start_tracing();
    let (_tp, tp_addr) = start_template_provider(None, DifficultyLevel::Low);
    let (_pool_1, pool_addr_1) = start_pool(sv2_tp_config(tp_addr), vec![], vec![]).await;
    let (_pool_2, pool_addr_2) = start_pool(sv2_tp_config(tp_addr), vec![], vec![]).await;

    let random_error_code = "Something went wrong".to_string();

    let open_mining_channel_success_replace = ReplaceMessage::new(
        MessageDirection::ToDownstream,
        MESSAGE_TYPE_OPEN_EXTENDED_MINING_CHANNEL_SUCCESS,
        AnyMessage::Mining(parsers_sv2::Mining::OpenMiningChannelError(
            OpenMiningChannelError {
                request_id: 0,
                error_code: random_error_code.try_into().unwrap(),
            },
        )),
    );

    let (pool_translator_sniffer_1, pool_translator_sniffer_addr_1) = start_sniffer(
        "A",
        pool_addr_1,
        false,
        vec![open_mining_channel_success_replace.into()],
        None,
    );

    let (pool_translator_sniffer_2, pool_translator_sniffer_addr_2) =
        start_sniffer("B", pool_addr_2, false, vec![], None);

    let (_, tproxy_addr) = start_sv2_translator(
        &[
            pool_translator_sniffer_addr_1,
            pool_translator_sniffer_addr_2,
        ],
        false,
        vec![],
        vec![],
        None,
    )
    .await;

    let (_minerd_process, _minerd_addr) = start_minerd(tproxy_addr, None, None, false).await;

    pool_translator_sniffer_1
        .wait_for_message_type(MessageDirection::ToUpstream, MESSAGE_TYPE_SETUP_CONNECTION)
        .await;
    pool_translator_sniffer_1
        .wait_for_message_type(
            MessageDirection::ToDownstream,
            MESSAGE_TYPE_SETUP_CONNECTION_SUCCESS,
        )
        .await;
    pool_translator_sniffer_1
        .wait_for_message_type(
            MessageDirection::ToUpstream,
            MESSAGE_TYPE_OPEN_EXTENDED_MINING_CHANNEL,
        )
        .await;

    pool_translator_sniffer_2
        .wait_for_message_type(MessageDirection::ToUpstream, MESSAGE_TYPE_SETUP_CONNECTION)
        .await;

    pool_translator_sniffer_2
        .wait_for_message_type(
            MessageDirection::ToDownstream,
            MESSAGE_TYPE_SETUP_CONNECTION_SUCCESS,
        )
        .await;

    pool_translator_sniffer_2
        .wait_for_message_type(
            MessageDirection::ToUpstream,
            MESSAGE_TYPE_OPEN_EXTENDED_MINING_CHANNEL,
        )
        .await;

    pool_translator_sniffer_2
        .wait_for_message_type(
            MessageDirection::ToDownstream,
            MESSAGE_TYPE_OPEN_EXTENDED_MINING_CHANNEL_SUCCESS,
        )
        .await;
}

// This test verifies that the translator sends keepalive jobs to downstream miners when no new
// jobs are received from upstream, and that shares submitted for keepalive jobs are properly
// received by the pool. Keepalive job_id(s) use the format `{original_job_id}#{counter}`.
#[tokio::test]
async fn test_translator_keepalive_job_sent_and_share_received_by_pool() {
    start_tracing();
    let (_tp, tp_addr) = start_template_provider(None, DifficultyLevel::High);
    let (_pool, pool_addr) = start_pool(sv2_tp_config(tp_addr), vec![], vec![]).await;
    let (pool_translator_sniffer, pool_translator_sniffer_addr) =
        start_sniffer("0", pool_addr, false, vec![], None);

    // Start translator with a short keepalive interval (5 seconds)
    let keepalive_interval_secs = 5_u16;
    let (_, tproxy_addr) = start_sv2_translator(
        &[pool_translator_sniffer_addr],
        false,
        vec![],
        vec![],
        Some(keepalive_interval_secs),
    )
    .await;
    let (sv1_sniffer, sv1_sniffer_addr) = start_sv1_sniffer(tproxy_addr);
    let (_minerd_process, _minerd_addr) = start_minerd(sv1_sniffer_addr, None, None, false).await;

    sv1_sniffer
        .wait_for_message(&["mining.notify"], &MessageDirection::ToDownstream)
        .await;

    pool_translator_sniffer
        .wait_for_message_type(
            MessageDirection::ToUpstream,
            MESSAGE_TYPE_SUBMIT_SHARES_EXTENDED,
        )
        .await;

    // Wait for keepalive interval plus some buffer time
    tokio::time::sleep(std::time::Duration::from_secs(
        keepalive_interval_secs as u64 + 3,
    ))
    .await;

    // Wait for a keepalive mining.notify message (job_id contains '#' delimiter)
    sv1_sniffer
        .wait_for_keepalive_notify(MessageDirection::ToDownstream)
        .await;

    // Wait for the share submission success message
    // This proves the keepalive job was valid and the share was properly mapped
    pool_translator_sniffer
        .wait_for_message_type(
            MessageDirection::ToDownstream,
            MESSAGE_TYPE_SUBMIT_SHARES_SUCCESS,
        )
        .await;
}
