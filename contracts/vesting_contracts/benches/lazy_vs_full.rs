use criterion::{black_box, criterion_group, criterion_main, Criterion};
use soroban_sdk::{vec, Env, Address, testutils::{Address as TestAddress}};
use vesting_contracts::{VestingContract, VestingContractClient, BatchCreateData};

fn bench_single_vault_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("single_vault_creation");
    
    // Full initialization benchmark
    group.bench_function("full_init", |b| {
        b.iter(|| {
            let env = Env::default();
            let contract_id = env.register(VestingContract, ());
            let client = VestingContractClient::new(&env, &contract_id);
            
            let admin = TestAddress::generate(&env);
            client.initialize(&admin, &1000000i128);
            
            let user = TestAddress::generate(&env);
            let start_time = 1640995200u64;
            let end_time = 1672531199u64;
            let amount = 100000i128;
            
            let vault_id = client.create_vault_full(
                black_box(&user),
                black_box(&amount),
                black_box(&start_time),
                black_box(&end_time)
            );
            
            black_box(vault_id);
        })
    });
    
    // Lazy initialization benchmark
    group.bench_function("lazy_init", |b| {
        b.iter(|| {
            let env = Env::default();
            let contract_id = env.register(VestingContract, ());
            let client = VestingContractClient::new(&env, &contract_id);
            
            let admin = TestAddress::generate(&env);
            client.initialize(&admin, &1000000i128);
            
            let user = TestAddress::generate(&env);
            let start_time = 1640995200u64;
            let end_time = 1672531199u64;
            let amount = 100000i128;
            
            let vault_id = client.create_vault_lazy(
                black_box(&user),
                black_box(&amount),
                black_box(&start_time),
                black_box(&end_time)
            );
            
            black_box(vault_id);
        })
    });
    
    group.finish();
}

fn bench_batch_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_creation");
    
    // Prepare batch data
    let env = Env::default();
    let mut recipients = vec![&env];
    let mut amounts = vec![&env];
    let mut start_times = vec![&env];
    let mut end_times = vec![&env];
    
    for i in 0..10 {
        recipients.push_back(TestAddress::generate(&env));
        amounts.push_back(100000i128);
        start_times.push_back(1640995200u64);
        end_times.push_back(1672531199u64);
    }
    
    // Full batch initialization benchmark
    group.bench_function("full_batch_10", |b| {
        b.iter(|| {
            let env = Env::default();
            let contract_id = env.register(VestingContract, ());
            let client = VestingContractClient::new(&env, &contract_id);
            
            let admin = TestAddress::generate(&env);
            client.initialize(&admin, &10000000i128);
            
            let batch_data = BatchCreateData {
                recipients: recipients.clone(),
                amounts: amounts.clone(),
                start_times: start_times.clone(),
                end_times: end_times.clone(),
            };
            
            let vault_ids = client.batch_create_vaults_full(black_box(&batch_data));
            black_box(vault_ids);
        })
    });
    
    // Lazy batch initialization benchmark
    group.bench_function("lazy_batch_10", |b| {
        b.iter(|| {
            let env = Env::default();
            let contract_id = env.register(VestingContract, ());
            let client = VestingContractClient::new(&env, &contract_id);
            
            let admin = TestAddress::generate(&env);
            client.initialize(&admin, &10000000i128);
            
            let batch_data = BatchCreateData {
                recipients: recipients.clone(),
                amounts: amounts.clone(),
                start_times: start_times.clone(),
                end_times: end_times.clone(),
            };
            
            let vault_ids = client.batch_create_vaults_lazy(black_box(&batch_data));
            black_box(vault_ids);
        })
    });
    
    group.finish();
}

fn bench_on_demand_initialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("on_demand_initialization");
    
    group.bench_function("initialize_10_vaults", |b| {
        b.iter(|| {
            let env = Env::default();
            let contract_id = env.register(VestingContract, ());
            let client = VestingContractClient::new(&env, &contract_id);
            
            let admin = TestAddress::generate(&env);
            client.initialize(&admin, &10000000i128);
            
            // Create lazy vaults
            let mut vault_ids = vec![&env];
            for i in 0..10 {
                let user = TestAddress::generate(&env);
                let vault_id = client.create_vault_lazy(&user, &100000i128, &1640995200u64, &1672531199u64);
                vault_ids.push_back(vault_id);
            }
            
            // Initialize on demand
            for vault_id in vault_ids.iter() {
                let vault = client.get_vault(vault_id);
                black_box(vault);
            }
        })
    });
    
    group.finish();
}

fn bench_large_batch_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("large_batch_creation");
    
    // Prepare large batch data (50 vaults)
    let env = Env::default();
    let mut recipients = vec![&env];
    let mut amounts = vec![&env];
    let mut start_times = vec![&env];
    let mut end_times = vec![&env];
    
    for i in 0..50 {
        recipients.push_back(TestAddress::generate(&env));
        amounts.push_back(50000i128);
        start_times.push_back(1640995200u64);
        end_times.push_back(1672531199u64);
    }
    
    // Full large batch initialization benchmark
    group.bench_function("full_batch_50", |b| {
        b.iter(|| {
            let env = Env::default();
            let contract_id = env.register(VestingContract, ());
            let client = VestingContractClient::new(&env, &contract_id);
            
            let admin = TestAddress::generate(&env);
            client.initialize(&admin, &50000000i128);
            
            let batch_data = BatchCreateData {
                recipients: recipients.clone(),
                amounts: amounts.clone(),
                start_times: start_times.clone(),
                end_times: end_times.clone(),
            };
            
            let vault_ids = client.batch_create_vaults_full(black_box(&batch_data));
            black_box(vault_ids);
        })
    });
    
    // Lazy large batch initialization benchmark
    group.bench_function("lazy_batch_50", |b| {
        b.iter(|| {
            let env = Env::default();
            let contract_id = env.register(VestingContract, ());
            let client = VestingContractClient::new(&env, &contract_id);
            
            let admin = TestAddress::generate(&env);
            client.initialize(&admin, &50000000i128);
            
            let batch_data = BatchCreateData {
                recipients: recipients.clone(),
                amounts: amounts.clone(),
                start_times: start_times.clone(),
                end_times: end_times.clone(),
            };
            
            let vault_ids = client.batch_create_vaults_lazy(black_box(&batch_data));
            black_box(vault_ids);
        })
    });
    
    group.finish();
}

criterion_group!(
    benches,
    bench_single_vault_creation,
    bench_batch_creation,
    bench_on_demand_initialization,
    bench_large_batch_creation
);
criterion_main!(benches);
