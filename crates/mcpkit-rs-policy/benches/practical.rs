//! Practical benchmark tests for real-world policy scenarios

use criterion::{BatchSize, Criterion, black_box, criterion_group, criterion_main};
use mcpkit_rs_policy::{
    cache::{ActionHash, check_with_cache, clear_cache},
    compiled::CompiledPolicy,
    permissions::{
        CorePermissions, EnvironmentPermissions, EnvironmentRule, NetworkPermissions, NetworkRule,
        Policy, ResourceLimitValues, ResourceLimits, StoragePermissions, StorageRule,
    },
};

/// Create a practical policy similar to what might be used in production
fn create_practical_policy() -> Policy {
    Policy {
        version: "1.0".to_string(),
        description: Some("Practical benchmark policy".to_string()),
        core: CorePermissions {
            storage: Some(StoragePermissions {
                allow: vec![
                    StorageRule {
                        uri: "fs:///home/user/**".to_string(),
                        access: vec!["read".to_string(), "write".to_string()],
                    },
                    StorageRule {
                        uri: "fs:///tmp/**".to_string(),
                        access: vec![
                            "read".to_string(),
                            "write".to_string(),
                            "execute".to_string(),
                        ],
                    },
                    StorageRule {
                        uri: "fs:///var/log/*.log".to_string(),
                        access: vec!["read".to_string()],
                    },
                ],
                deny: vec![
                    StorageRule {
                        uri: "fs:///etc/shadow".to_string(),
                        access: vec!["read".to_string(), "write".to_string()],
                    },
                    StorageRule {
                        uri: "fs:///root/**".to_string(),
                        access: vec![
                            "read".to_string(),
                            "write".to_string(),
                            "execute".to_string(),
                        ],
                    },
                ],
            }),
            network: Some(NetworkPermissions {
                allow: vec![
                    NetworkRule::Host {
                        host: "api.github.com".to_string(),
                    },
                    NetworkRule::Host {
                        host: "*.googleapis.com".to_string(),
                    },
                    NetworkRule::Host {
                        host: "localhost".to_string(),
                    },
                    NetworkRule::Host {
                        host: "127.0.0.1".to_string(),
                    },
                ],
                deny: vec![
                    NetworkRule::Host {
                        host: "malware.com".to_string(),
                    },
                    NetworkRule::Host {
                        host: "*.suspicious.org".to_string(),
                    },
                ],
            }),
            environment: Some(EnvironmentPermissions {
                allow: vec![
                    EnvironmentRule {
                        key: "HOME".to_string(),
                    },
                    EnvironmentRule {
                        key: "PATH".to_string(),
                    },
                    EnvironmentRule {
                        key: "USER".to_string(),
                    },
                    EnvironmentRule {
                        key: "LANG".to_string(),
                    },
                    EnvironmentRule {
                        key: "TERM".to_string(),
                    },
                ],
                deny: vec![
                    EnvironmentRule {
                        key: "AWS_SECRET_ACCESS_KEY".to_string(),
                    },
                    EnvironmentRule {
                        key: "DATABASE_PASSWORD".to_string(),
                    },
                ],
            }),
            resources: Some(ResourceLimits {
                limits: ResourceLimitValues {
                    cpu: Some("500m".to_string()),
                    memory: Some("256Mi".to_string()),
                    execution_time: Some("30s".to_string()),
                    fuel: Some(1_000_000),
                    memory_limit: None,
                },
            }),
        },
        extensions: Default::default(),
    }
}

fn bench_practical_mixed_checks(c: &mut Criterion) {
    let policy = create_practical_policy();
    let compiled = CompiledPolicy::compile(&policy).expect("Failed to compile policy");

    c.bench_function("practical_storage_allowed", |b| {
        b.iter(|| black_box(compiled.is_storage_allowed("/home/user/documents/file.txt", "read")))
    });

    c.bench_function("practical_storage_denied", |b| {
        b.iter(|| black_box(compiled.is_storage_allowed("/etc/shadow", "read")))
    });

    c.bench_function("practical_storage_glob_match", |b| {
        b.iter(|| black_box(compiled.is_storage_allowed("/var/log/system.log", "read")))
    });

    c.bench_function("practical_network_allowed", |b| {
        b.iter(|| black_box(compiled.is_network_allowed("api.github.com")))
    });

    c.bench_function("practical_network_glob_match", |b| {
        b.iter(|| black_box(compiled.is_network_allowed("maps.googleapis.com")))
    });

    c.bench_function("practical_network_denied", |b| {
        b.iter(|| black_box(compiled.is_network_allowed("malware.com")))
    });
}

fn bench_practical_with_cache(c: &mut Criterion) {
    let policy = create_practical_policy();
    let compiled = CompiledPolicy::compile(&policy).expect("Failed to compile policy");
    let compiled_arc = std::sync::Arc::new(compiled);

    c.bench_function("practical_cached_storage_check", |b| {
        // Clear cache before benchmark
        clear_cache();
        let compiled_clone = compiled_arc.clone();

        b.iter(|| {
            let action = ActionHash::Storage("/home/user/test.txt".into(), "read".to_string());
            black_box(check_with_cache(action, || {
                Ok(compiled_clone.is_storage_allowed("/home/user/test.txt", "read"))
            }))
        })
    });

    c.bench_function("practical_cache_hit_rate", |b| {
        clear_cache();
        let compiled_clone = compiled_arc.clone();

        b.iter_batched(
            || {
                // Setup: clear cache
                clear_cache();
                // Warm up the cache with a few entries
                for i in 0..10 {
                    let action = ActionHash::Storage(
                        format!("/tmp/file{}.txt", i).into(),
                        "read".to_string(),
                    );
                    let _ = check_with_cache(action, || {
                        Ok(compiled_clone
                            .is_storage_allowed(&format!("/tmp/file{}.txt", i), "read"))
                    });
                }
            },
            |_| {
                // Benchmark: check a cached item
                let action = ActionHash::Storage("/tmp/file5.txt".into(), "read".to_string());
                black_box(check_with_cache(action, || {
                    Ok(compiled_clone.is_storage_allowed("/tmp/file5.txt", "read"))
                }))
            },
            BatchSize::SmallInput,
        )
    });
}

fn bench_practical_compilation(c: &mut Criterion) {
    c.bench_function("practical_policy_compilation", |b| {
        let policy = create_practical_policy();
        b.iter(|| black_box(CompiledPolicy::compile(&policy).expect("Failed to compile")))
    });
}

criterion_group!(
    practical_benches,
    bench_practical_mixed_checks,
    bench_practical_with_cache,
    bench_practical_compilation
);
criterion_main!(practical_benches);
