//! Main entry point for Cli

#![deny(warnings, missing_docs, trivial_casts, unused_qualifications)]
#![forbid(unsafe_code)]

use ibc::{
    applications::transfer::MODULE_ID_STR as IBC_TRANSFER_MODULE_ID,
    core::{ics24_host::identifier::PortId, ics26_routing::context::ModuleId},
};
use ibc_proto::cosmos::{
    base::tendermint::v1beta1::service_server::ServiceServer as HealthServer,
    tx::v1beta1::service_server::ServiceServer as TxServer,
};
use structopt::StructOpt;
use tendermint_abci::ServerBuilder;
use tendermint_basecoin::{
    app::Builder,
    cli::option::Opt,
    modules::{prefix, Identifiable, Module},
    modules::{Auth, Bank, Ibc, IbcTransferModule, Staking},
    store::memory::InMemoryStore,
};
use tokio::runtime::Runtime;
use tonic::transport::Server;
use tracing_subscriber::filter::LevelFilter;

fn main() {
    let opt: Opt = Opt::from_args();
    let log_level = if opt.quiet {
        LevelFilter::OFF
    } else if opt.verbose {
        LevelFilter::TRACE
    } else {
        LevelFilter::INFO
    };
    tracing_subscriber::fmt().with_max_level(log_level).init();
    tracing::info!("Starting app and waiting for Tendermint to connect...");

    // instantiate the application with a KV store implementation of choice
    let app_builder = Builder::new(InMemoryStore::default());

    // instantiate modules and setup inter-module communication (if required)
    let auth = Auth::new(app_builder.module_store(&prefix::Auth {}.identifier()));
    let bank = Bank::new(
        app_builder.module_store(&prefix::Bank {}.identifier()),
        auth.account_reader().clone(),
        auth.account_keeper().clone(),
    );
    let staking = Staking::new(app_builder.module_store(&prefix::Staking {}.identifier()));

    let ibc = {
        let mut ibc = Ibc::new(app_builder.module_store(&prefix::Ibc {}.identifier()));

        let transfer_module_id: ModuleId = IBC_TRANSFER_MODULE_ID.parse().unwrap();
        let module = IbcTransferModule::new(ibc.store().clone(), bank.bank_keeper().clone());
        ibc.add_route(transfer_module_id.clone(), module).unwrap();

        ibc.scope_port_to_module(PortId::transfer(), transfer_module_id);

        ibc
    };

    // instantiate gRPC services for each module
    let auth_service = auth.service();
    let bank_service = bank.service();
    let staking_service = staking.service();
    let ibc_client_service = ibc.client_service();
    let ibc_conn_service = ibc.connection_service();
    let ibc_channel_service = ibc.channel_service();

    // register modules with the app
    let app = app_builder
        .add_module(prefix::Auth {}.identifier(), auth)
        .add_module(prefix::Bank {}.identifier(), bank)
        .add_module(prefix::Ibc {}.identifier(), ibc)
        .build();

    // run the blocking ABCI server on a separate thread
    let server = ServerBuilder::new(opt.read_buf_size)
        .bind(format!("{}:{}", opt.host, opt.port), app.clone())
        .unwrap();
    std::thread::spawn(move || {
        server.listen().unwrap();
    });

    // run the gRPC server
    let grpc_server = Server::builder()
        .add_service(HealthServer::new(app.clone()))
        .add_service(TxServer::new(app))
        .add_service(ibc_client_service)
        .add_service(ibc_conn_service)
        .add_service(ibc_channel_service)
        .add_service(auth_service)
        .add_service(bank_service)
        .add_service(staking_service)
        .serve(format!("{}:{}", opt.host, opt.grpc_port).parse().unwrap());
    Runtime::new()
        .unwrap()
        .block_on(async { grpc_server.await.unwrap() });
}