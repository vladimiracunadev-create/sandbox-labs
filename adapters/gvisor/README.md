# Adaptador gVisor

Requiere `runsc` y una carga OCI preparada. Antes de habilitarlo:

1. valida que `runsc doctor` sea satisfactorio;
2. usa un bundle OCI efímero;
3. elimina capabilities;
4. deshabilita red salvo necesidad explícita;
5. captura la configuración efectiva en la evidencia.
