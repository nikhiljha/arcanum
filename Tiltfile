allow_k8s_contexts('kubernetes-admin@kubernetes')

load('ext://kubectl_build', 'kubectl_build')
local_resource('compile', 'make compile')
kubectl_build('njha/arcanum', '.', dockerfile='Dockerfile')
k8s_yaml('yaml/deployment.yaml')
k8s_resource('arcanum', port_forwards=8080)
