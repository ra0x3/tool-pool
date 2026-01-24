//! Simple benchmark tests for basic policy decisions

use criterion::{Criterion, black_box, criterion_group, criterion_main};
use mcpkit_rs_policy::{
    compiled::CompiledPolicy,
    permissions::{CorePermissions, EnvironmentPermissions, EnvironmentRule, Policy},
};

/// Create a minimal policy with just a few environment variable permissions
fn create_simple_policy() -> Policy {
    Policy {
        version: "1.0".to_string(),
        description: Some("Simple benchmark policy".to_string()),
        core: CorePermissions {
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
                ],
                deny: vec![EnvironmentRule {
                    key: "SECRET_KEY".to_string(),
                }],
            }),
            ..Default::default()
        },
        extensions: Default::default(),
    }
}

fn bench_simple_env_check(c: &mut Criterion) {
    let policy = create_simple_policy();
    let compiled = CompiledPolicy::compile(&policy).expect("Failed to compile policy");

    c.bench_function("simple_env_allowed", |b| {
        b.iter(|| {
            // Check an allowed environment variable
            black_box(compiled.is_env_allowed("HOME"))
        })
    });

    c.bench_function("simple_env_denied", |b| {
        b.iter(|| {
            // Check a denied environment variable
            black_box(compiled.is_env_allowed("SECRET_KEY"))
        })
    });

    c.bench_function("simple_env_not_listed", |b| {
        b.iter(|| {
            // Check an environment variable that's not in the policy
            black_box(compiled.is_env_allowed("RANDOM_VAR"))
        })
    });
}

fn bench_simple_compilation(c: &mut Criterion) {
    c.bench_function("simple_policy_compilation", |b| {
        let policy = create_simple_policy();
        b.iter(|| black_box(CompiledPolicy::compile(&policy).expect("Failed to compile")))
    });
}

criterion_group!(
    simple_benches,
    bench_simple_env_check,
    bench_simple_compilation
);
criterion_main!(simple_benches);
