use crate::{
    cluster_api::{
        clusterclasses::{
            ClusterClassPatches, ClusterClassPatchesDefinitions,
            ClusterClassPatchesDefinitionsJsonPatches,
            ClusterClassPatchesDefinitionsJsonPatchesValueFrom,
            ClusterClassPatchesDefinitionsSelector,
            ClusterClassPatchesDefinitionsSelectorMatchResources, ClusterClassVariables,
            ClusterClassVariablesSchema,
        },
        kubeadmcontrolplanetemplates::{
            KubeadmControlPlaneTemplate,
            KubeadmControlPlaneTemplateTemplateSpecKubeadmConfigSpecFiles,
        },
    },
    features::{
        ClusterClassVariablesSchemaExt, ClusterFeatureEntry, ClusterFeaturePatches,
        ClusterFeatureVariables,
    },
};
use cluster_feature_derive::ClusterFeatureValues;
use indoc::indoc;
use kube::CustomResourceExt;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
struct Kustomize {
    resources: Vec<String>,
    patches: Vec<KustomizePatch>,
}

#[derive(Debug, Serialize, Deserialize)]
struct KustomizePatchTarget {
    group: String,
    version: String,
    kind: String,
    name: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct KustomizePatch {
    target: KustomizePatchTarget,
    patch: String,
}

#[derive(Serialize, Deserialize, ClusterFeatureValues)]
#[allow(dead_code)]
pub struct FeatureValues {
    #[serde(rename = "kubeAPIOptions")]
    pub kubeapi_options: Vec<String>,
}

pub struct Feature {}

impl ClusterFeaturePatches for Feature {
    fn patches(&self) -> Vec<ClusterClassPatches> {
        vec![ClusterClassPatches {
            name: "kubeAPIOptions".into(),
            enabled_if: Some("{{ if (index .kubeAPIOptions 0) }}true{{ end }}".into()),
            definitions: Some(vec![ClusterClassPatchesDefinitions {
                selector: ClusterClassPatchesDefinitionsSelector {
                    api_version: KubeadmControlPlaneTemplate::api_resource().api_version,
                    kind: KubeadmControlPlaneTemplate::api_resource().kind,
                    match_resources: ClusterClassPatchesDefinitionsSelectorMatchResources {
                        control_plane: Some(true),
                        ..Default::default()
                    },
                },
                json_patches: vec![
                    ClusterClassPatchesDefinitionsJsonPatches {
                        op: "add".into(),
                        path: "/spec/template/spec/kubeadmConfigSpec/files/-".into(),
                        value_from: Some(ClusterClassPatchesDefinitionsJsonPatchesValueFrom {
                            template: Some(serde_json::to_string(&KubeadmControlPlaneTemplateTemplateSpecKubeadmConfigSpecFiles {
                                path: "/etc/kubernetes/kustomizations/kubeapi_options/kustomization.yml".into(),
                                permissions: Some("0644".into()),
                                owner: Some("root:root".into()),
                                content: Some(serde_yaml::to_string(&Kustomize {
                                    resources: vec!["kube-apiserver.yaml".into()],
                                    patches: vec![
                                        KustomizePatch {
                                            target: KustomizePatchTarget {
                                                group: "".into(),
                                                version: "v1".into(),
                                                kind: "Pod".into(),
                                                name: "kube-apiserver".into(),
                                            },
                                            patch: indoc!(r#"
                                                {{- range .kubeAPIOptions }}
                                                - op: add
                                                  path: /spec/containers/0/command/-
                                                  value: {{ . }}
                                                {{ end -}}
                                            "#).into(),
                                        },
                                    ]
                                }).unwrap()),
                                ..Default::default()
                            }).unwrap()),
                            variable: None,
                        }),
                        ..Default::default()
                    },
                    ClusterClassPatchesDefinitionsJsonPatches {
                        op: "add".into(),
                        path: "/spec/template/spec/kubeadmConfigSpec/preKubeadmCommands/-".into(),
                        value: Some("mkdir -p /etc/kubernetes/kustomizations/kubeapi_options".into()),
                        ..Default::default()
                    },
                    ClusterClassPatchesDefinitionsJsonPatches {
                        op: "add".into(),
                        path: "/spec/template/spec/kubeadmConfigSpec/postKubeadmCommands/-".into(),
                        value: Some("cp /etc/kubernetes/manifests/kube-apiserver.yaml /etc/kubernetes/kustomizations/kubeapi_options/kube-apiserver.yaml".into()),
                        ..Default::default()
                    },
                    ClusterClassPatchesDefinitionsJsonPatches {
                        op: "add".into(),
                        path: "/spec/template/spec/kubeadmConfigSpec/postKubeadmCommands/-".into(),
                        value: Some("kubectl kustomize /etc/kubernetes/kustomizations/kubeapi_options -o /etc/kubernetes/manifests/kube-apiserver.yaml".into()),
                        ..Default::default()
                    },
                ],
            }]),
            ..Default::default()
        }]
    }
}

inventory::submit! {
    ClusterFeatureEntry{ feature: &Feature {} }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::test::{ApplyPatch, TestClusterResources};
    use crate::resources::fixtures::default_values;
    use k8s_openapi::api::core::v1::Pod;
    use pretty_assertions::assert_eq;
    use std::fs::File;

    #[test]
    fn test_empty_patch_args() {
        let feature = Feature {};

        let mut values = default_values();
        values.kubeapi_options = vec!["".into()];

        let patches = feature.patches();
        let mut resources = TestClusterResources::new();
        resources.apply_patches(&patches, &values);

        let files = resources
            .kubeadm_control_plane_template
            .spec
            .template
            .spec
            .kubeadm_config_spec
            .files
            .expect("files should be set");

        assert_eq!(
            files
                .iter()
                .find(|f| f.path == "/etc/kubernetes/kustomizations/kubeapi_options/kustomization.yml"),
            None
        );

        let post_cmds = resources
            .kubeadm_control_plane_template
            .spec
            .template
            .spec
            .kubeadm_config_spec
            .post_kubeadm_commands
            .expect("post commands should be set");

        assert_eq!(
            post_cmds.contains(
                &"cp /etc/kubernetes/manifests/kube-apiserver.yaml /etc/kubernetes/kustomizations/kubeapi_options/kube-apiserver.yaml".to_string()
            ),
            false,
            "postKubeadmCommands should not contain references to kubeapi_options kustomizations",
        );
    }

    #[test]
    fn test_apply_patches() {
        let feature = Feature {};

        let mut values = default_values();
        values.kubeapi_options = vec!["--foo=1".into(), "--bar=2".into()];

        let patches = feature.patches();
        let mut resources = TestClusterResources::new();
        resources.apply_patches(&patches, &values);

        let files = resources
            .kubeadm_control_plane_template
            .spec
            .template
            .spec
            .kubeadm_config_spec
            .files
            .expect("files should be set");

        let file = files
            .iter()
            .find(|f| f.path == "/etc/kubernetes/kustomizations/kubeapi_options/kustomization.yml")
            .expect("file should be set");

        assert_eq!(
            file.path,
            "/etc/kubernetes/kustomizations/kubeapi_options/kustomization.yml"
        );
        assert_eq!(file.permissions.as_deref(), Some("0644"));
        assert_eq!(file.owner.as_deref(), Some("root:root"));
        assert!(file.content.is_some());

        let path = format!(
            "{}/tests/fixtures/kube-apiserver.yaml",
            env!("CARGO_MANIFEST_DIR")
        );
        let fd = File::open(&path).expect("file should be set");
        let mut pod: Pod = serde_yaml::from_reader(fd).expect("pod should be set");
        let kustomize: Kustomize =
            serde_yaml::from_str(file.content.as_ref().unwrap()).expect("kustomize should be set");
        let patch = serde_yaml::from_str(&kustomize.patches[0].patch).expect("patch should be set");
        pod.apply_patch(&patch);

        let args = pod.spec.expect("pod to have spec").containers[0]
            .command
            .clone()
            .expect("command should be set");

        assert!(args.contains(&"--foo=1".to_string()));
        assert!(args.contains(&"--bar=2".to_string()));

        let pre_cmds = resources
            .kubeadm_control_plane_template
            .spec
            .template
            .spec
            .kubeadm_config_spec
            .pre_kubeadm_commands
            .expect("pre commands should be set");
        assert!(pre_cmds.contains(&"mkdir -p /etc/kubernetes/kustomizations/kubeapi_options".to_string()));

        let post_cmds = resources
            .kubeadm_control_plane_template
            .spec
            .template
            .spec
            .kubeadm_config_spec
            .post_kubeadm_commands
            .expect("post commands should be set");
        assert!(post_cmds.contains(&"cp /etc/kubernetes/manifests/kube-apiserver.yaml /etc/kubernetes/kustomizations/kubeapi_options/kube-apiserver.yaml".to_string()));
        assert!(post_cmds.contains(&"kubectl kustomize /etc/kubernetes/kustomizations/kubeapi_options -o /etc/kubernetes/manifests/kube-apiserver.yaml".to_string()));
    }
}
