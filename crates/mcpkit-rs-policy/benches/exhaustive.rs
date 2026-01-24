//! Exhaustive benchmark tests for stress-testing the policy engine

use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use mcpkit_rs_policy::{
    cache::{ActionHash, check_with_cache},
    compiled::CompiledPolicy,
    permissions::{
        CorePermissions, EnvironmentPermissions, EnvironmentRule, NetworkPermissions, NetworkRule,
        Policy, ResourceLimitValues, ResourceLimits, StoragePermissions, StorageRule,
    },
};

/// Create an exhaustive policy with many rules and patterns
fn create_exhaustive_policy() -> Policy {
    // Generate hundreds of rules to stress test the engine
    let mut storage_allow_rules = Vec::new();
    let mut storage_deny_rules = Vec::new();
    let mut network_allow_rules = Vec::new();
    let mut network_deny_rules = Vec::new();
    let mut env_allow_rules = Vec::new();
    let mut env_deny_rules = Vec::new();

    // Add many storage rules with various patterns
    for i in 0..100 {
        storage_allow_rules.push(StorageRule {
            uri: format!("fs:///data/project{}/src/**/*.rs", i),
            access: vec!["read".to_string(), "write".to_string()],
        });
        storage_allow_rules.push(StorageRule {
            uri: format!("fs:///tmp/cache/layer{}/{{a,b,c}}/**", i),
            access: vec!["read".to_string()],
        });
        storage_deny_rules.push(StorageRule {
            uri: format!("fs:///secure/vault{}/secrets/*", i),
            access: vec![
                "read".to_string(),
                "write".to_string(),
                "execute".to_string(),
            ],
        });
    }

    // Add many network rules with glob patterns
    for i in 0..50 {
        network_allow_rules.push(NetworkRule::Host {
            host: format!("api{}.example.com", i),
        });
        network_allow_rules.push(NetworkRule::Host {
            host: format!("*.service{}.cloud.com", i),
        });
        network_deny_rules.push(NetworkRule::Host {
            host: format!("malicious{}.badsite.org", i),
        });
        network_deny_rules.push(NetworkRule::Host {
            host: format!("*.tracker{}.analytics.net", i),
        });
    }

    // Add many environment variables
    for i in 0..200 {
        env_allow_rules.push(EnvironmentRule {
            key: format!("APP_CONFIG_{}", i),
        });
        if i % 3 == 0 {
            env_deny_rules.push(EnvironmentRule {
                key: format!("SECRET_KEY_{}", i),
            });
        }
    }

    Policy {
        version: "1.0".to_string(),
        description: Some("Exhaustive benchmark policy".to_string()),
        core: CorePermissions {
            storage: Some(StoragePermissions {
                allow: storage_allow_rules,
                deny: storage_deny_rules,
            }),
            network: Some(NetworkPermissions {
                allow: network_allow_rules,
                deny: network_deny_rules,
            }),
            environment: Some(EnvironmentPermissions {
                allow: env_allow_rules,
                deny: env_deny_rules,
            }),
            resources: Some(ResourceLimits {
                limits: ResourceLimitValues {
                    cpu: Some("2000m".to_string()),
                    memory: Some("4Gi".to_string()),
                    execution_time: Some("5m".to_string()),
                    fuel: Some(100_000_000),
                    memory_limit: None,
                },
            }),
        },
        extensions: Default::default(),
    }
}

fn bench_exhaustive_glob_patterns(c: &mut Criterion) {
    let policy = create_exhaustive_policy();
    let compiled = CompiledPolicy::compile(&policy).expect("Failed to compile policy");

    // Test various path depths and patterns
    let test_paths = vec![
        ("/data/project50/src/main.rs", "read"),
        (
            "/data/project99/src/lib/deep/nested/module/file.rs",
            "write",
        ),
        (
            "/tmp/cache/layer25/a/very/deep/nested/directory/structure/file.txt",
            "read",
        ),
        ("/secure/vault75/secrets/password.txt", "read"),
        ("/random/path/that/does/not/match/any/pattern.txt", "read"),
    ];

    for (path, op) in test_paths {
        c.bench_function(
            &format!("exhaustive_storage_{}", path.split('/').last().unwrap()),
            |b| b.iter(|| black_box(compiled.is_storage_allowed(path, op))),
        );
    }
}

fn bench_exhaustive_network_patterns(c: &mut Criterion) {
    let policy = create_exhaustive_policy();
    let compiled = CompiledPolicy::compile(&policy).expect("Failed to compile policy");

    // Test various host patterns
    let test_hosts = vec![
        "api25.example.com",
        "backend.service49.cloud.com",
        "malicious37.badsite.org",
        "tracking.tracker15.analytics.net",
        "unknown.random.domain.com",
        "deep.subdomain.service10.cloud.com",
    ];

    for host in test_hosts {
        c.bench_function(
            &format!("exhaustive_network_{}", host.split('.').next().unwrap()),
            |b| b.iter(|| black_box(compiled.is_network_allowed(host))),
        );
    }
}

fn bench_exhaustive_cache_stress(c: &mut Criterion) {
    let policy = create_exhaustive_policy();
    let compiled = CompiledPolicy::compile(&policy).expect("Failed to compile policy");
    let compiled_arc = std::sync::Arc::new(compiled);

    // Benchmark cache performance under heavy load
    c.bench_function("exhaustive_cache_thrashing", |b| {
        let compiled_clone = compiled_arc.clone();
        let mut counter = 0;

        b.iter(|| {
            // Generate different actions to stress the cache
            let action = match counter % 4 {
                0 => ActionHash::Storage(
                    format!("/data/project{}/file.rs", counter % 100).into(),
                    "read".to_string(),
                ),
                1 => ActionHash::Network(format!("api{}.example.com", counter % 50)),
                2 => ActionHash::Environment(format!("APP_CONFIG_{}", counter % 200)),
                _ => ActionHash::Tool(format!("tool_{}", counter % 30)),
            };

            counter += 1;

            black_box(check_with_cache(action, || match counter % 4 {
                0 => Ok(compiled_clone.is_storage_allowed(
                    &format!("/data/project{}/file.rs", (counter - 1) % 100),
                    "read",
                )),
                1 => Ok(compiled_clone
                    .is_network_allowed(&format!("api{}.example.com", (counter - 1) % 50))),
                2 => {
                    Ok(compiled_clone
                        .is_env_allowed(&format!("APP_CONFIG_{}", (counter - 1) % 200)))
                }
                _ => Ok(compiled_clone.is_tool_allowed(&format!("tool_{}", (counter - 1) % 30))),
            }))
        })
    });
}

fn bench_exhaustive_worst_case(c: &mut Criterion) {
    let policy = create_exhaustive_policy();
    let compiled = CompiledPolicy::compile(&policy).expect("Failed to compile policy");

    // Benchmark the absolute worst case: checking something that requires
    // traversing all rules and doesn't match any pattern
    c.bench_function("exhaustive_worst_case_no_match", |b| {
        b.iter(|| {
            // These should not match any pattern, forcing full traversal
            black_box(compiled.is_storage_allowed(
                "/completely/unrelated/path/that/will/never/match/any/configured/pattern/file.xyz",
                "execute",
            ));
            black_box(compiled.is_network_allowed(
                "never.gonna.give.you.up.never.gonna.let.you.down.rickroll.com",
            ));
            black_box(
                compiled.is_env_allowed("SUPER_DUPER_UNLIKELY_ENV_VAR_NAME_THAT_NOBODY_WOULD_USE"),
            );
        })
    });
}

fn bench_exhaustive_compilation_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("exhaustive_compilation_scaling");

    // Test compilation time with increasingly complex policies
    for size in [10, 50, 100, 200, 500].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            // Create a policy with 'size' rules per category
            let mut storage_rules = Vec::new();
            let mut network_rules = Vec::new();
            let mut env_rules = Vec::new();

            for i in 0..size {
                storage_rules.push(StorageRule {
                    uri: format!("fs:///path{}/file*.txt", i),
                    access: vec!["read".to_string()],
                });
                network_rules.push(NetworkRule::Host {
                    host: format!("host{}.example.com", i),
                });
                env_rules.push(EnvironmentRule {
                    key: format!("VAR_{}", i),
                });
            }

            let policy = Policy {
                version: "1.0".to_string(),
                description: None,
                core: CorePermissions {
                    storage: Some(StoragePermissions {
                        allow: storage_rules,
                        deny: vec![],
                    }),
                    network: Some(NetworkPermissions {
                        allow: network_rules,
                        deny: vec![],
                    }),
                    environment: Some(EnvironmentPermissions {
                        allow: env_rules,
                        deny: vec![],
                    }),
                    resources: None,
                },
                extensions: Default::default(),
            };

            b.iter(|| black_box(CompiledPolicy::compile(&policy).expect("Failed to compile")))
        });
    }
    group.finish();
}

fn bench_exhaustive_concurrent_patterns(c: &mut Criterion) {
    let policy = create_exhaustive_policy();
    let compiled = CompiledPolicy::compile(&policy).expect("Failed to compile policy");
    let compiled_arc = std::sync::Arc::new(compiled);

    // Simulate concurrent access patterns
    c.bench_function("exhaustive_mixed_concurrent_patterns", |b| {
        let compiled_clone = compiled_arc.clone();
        let patterns = vec![
            ("storage", "/data/project42/src/lib.rs", "read"),
            ("network", "api15.example.com", ""),
            ("env", "APP_CONFIG_100", ""),
            ("storage", "/tmp/cache/layer10/b/deep/file.txt", "write"),
            ("network", "backend.service25.cloud.com", ""),
            ("storage", "/secure/vault50/secrets/key.pem", "read"),
            ("env", "SECRET_KEY_99", ""),
            ("network", "malicious20.badsite.org", ""),
        ];

        let mut index = 0;
        b.iter(|| {
            let (check_type, resource, operation) = &patterns[index % patterns.len()];
            index += 1;

            match *check_type {
                "storage" => black_box(compiled_clone.is_storage_allowed(resource, operation)),
                "network" => black_box(compiled_clone.is_network_allowed(resource)),
                "env" => black_box(compiled_clone.is_env_allowed(resource)),
                _ => unreachable!(),
            }
        })
    });
}

criterion_group!(
    exhaustive_benches,
    bench_exhaustive_glob_patterns,
    bench_exhaustive_network_patterns,
    bench_exhaustive_cache_stress,
    bench_exhaustive_worst_case,
    bench_exhaustive_compilation_scaling,
    bench_exhaustive_concurrent_patterns
);
criterion_main!(exhaustive_benches);
