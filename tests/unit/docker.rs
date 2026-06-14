use super::*;

#[test]
fn parses_docker_ps_output() {
    let output = "c22bd1e7a321\tweb\tnginx:alpine\n\
                  4720e895aae1\tworker\tapp:latest\n";

    let containers = parse_containers(output);

    assert_eq!(
        containers,
        vec![
            Container {
                id: "c22bd1e7a321".to_string(),
                name: "web".to_string(),
                image: "nginx:alpine".to_string(),
            },
            Container {
                id: "4720e895aae1".to_string(),
                name: "worker".to_string(),
                image: "app:latest".to_string(),
            },
        ]
    );
}

#[test]
fn skips_invalid_docker_ps_lines() {
    let output = "missing-fields\n\
                  \tno-id\timage\n\
                  id\t\timage\n\
                  c22bd1e7a321\tweb\t\n";

    let containers = parse_containers(output);

    assert_eq!(
        containers,
        vec![Container {
            id: "c22bd1e7a321".to_string(),
            name: "web".to_string(),
            image: "unknown".to_string(),
        }]
    );
}

#[test]
fn configured_shell_is_preferred_and_deduped() {
    let candidates = shell_candidates(Some("/bin/bash"));

    assert_eq!(candidates[0], "/bin/bash");
    assert_eq!(
        candidates
            .iter()
            .filter(|candidate| candidate.as_str() == "/bin/bash")
            .count(),
        1
    );
}

#[test]
fn shell_env_ignores_non_shell_values() {
    assert_eq!(parse_shell_env("PATH=/bin\nSHELL=/sbin/nologin\n"), None);
    assert_eq!(
        parse_shell_env("PATH=/bin\nSHELL=/bin/zsh\n").as_deref(),
        Some("/bin/zsh")
    );
}

#[test]
fn shell_command_runs_docker_exec_with_tty() {
    let container = Container {
        id: "c22bd1e7a321".to_string(),
        name: "web".to_string(),
        image: "nginx:alpine".to_string(),
    };

    assert_eq!(
        shell_command_with_shell(&container, "/bin/bash"),
        "docker exec -it 'c22bd1e7a321' '/bin/bash'"
    );
}
