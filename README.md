# PARAMO

PARAMO es un bloqueador de distracciones para macOS, escrito en Rust. Gestiona una sección propia dentro de `/etc/hosts`, ofrece CLI y TUI, y puede quedar instalado como daemon de `launchd` para aplicar el bloqueo por horario sin tener que abrir la app manualmente. 


## Instalación con Homebrew

```bash
brew tap attinto/paramo
brew install paramo
sudo paramo install
paramo doctor
```

También puedes instalarlo en un solo comando:

```bash
brew install attinto/paramo/paramo
```

Homebrew descarga un binario precompilado (bottle) para tu arquitectura. No necesita compilar Rust. Si no hay bottle disponible para tu versión de macOS, compilará desde fuente como fallback automático.

Homebrew instala solo el binario CLI. La integración con `/etc/hosts` y `launchd` es un paso explícito aparte:

```bash
sudo paramo install
```

Después de eso, las acciones diarias del CLI y de la TUI funcionan sin `sudo`, delegando en el daemon instalado.

## Actualizar con Homebrew

Para actualizar el binario:

```bash
brew update
brew upgrade paramo
```

Si una release cambia el daemon, el plist o la configuración del sistema, vuelve a ejecutar:

```bash
sudo paramo install
```

## Desinstalar

Para quitar la integración de sistema:

```bash
sudo paramo uninstall
```

Para quitar el binario instalado por Homebrew:

```bash
brew uninstall paramo
```

Si quieres eliminar ambas cosas, ejecuta los dos comandos.

## Uso rápido

### Abrir la TUI

```bash
paramo
```

### Ver estado

```bash
paramo status
```

### Bloquear y desbloquear manualmente

```bash
paramo block
paramo unblock
```

### Gestionar sitios

```bash
paramo site list
paramo site add youtube.com
paramo site remove youtube.com
```

### Ver y cambiar el horario

```bash
paramo schedule show
paramo schedule set --start 9 --end 18 --weekends off
paramo schedule set --start 22 --end 8 --weekends on
```

### Idioma

```bash
paramo lang show
paramo lang set es
paramo lang set en
```

### Ver la configuración efectiva

```bash
paramo config show
```

### Diagnóstico

```bash
paramo doctor
```

## Desarrollo local

Requisitos:

- macOS
- Rust estable
- Homebrew si quieres probar la fórmula
- permisos de administrador para `paramo install` y `paramo uninstall`

Compilar:

```bash
cargo build --release
```

## Actualizar rapido en tu terminal sin release

Si solo quieres usar la version nueva en tu Mac, sin PR, sin tag y sin Homebrew, tienes dos opciones:

### Opcion 1: probar al instante desde el repo

```bash
cargo run --release -- status
cargo run --release -- doctor
```

O abrir directamente la app compilada:

```bash
cargo build --release
./target/release/paramo
```

### Opcion 2: reemplazar tu binario local global

```bash
cargo install --path . --force
```

Despues puedes comprobar que `paramo` apunta al binario correcto:

```bash
which -a paramo
```

Si el de Homebrew sigue apareciendo antes que el de Cargo, usa directamente:

```bash
~/.cargo/bin/paramo
```

Si esta version cambia daemon, plist o configuracion del sistema, vuelve a instalar la parte de sistema con el binario local:

```bash
sudo ~/.cargo/bin/paramo install
```

Validar el proyecto:

```bash
cargo test
cargo clippy --all-targets -- -D warnings
```

Instalar la parte del sistema desde el binario local:

```bash
sudo ./target/release/paramo install
```

Verificar:

```bash
paramo doctor
paramo status
sudo launchctl print system/com.paramo.blocker
```

## Fórmula Homebrew

La fórmula vive en `Formula/paramo.rb`.

Cada release publica bottles precompilados para arm64 (Apple Silicon) e x86_64 (Intel), generados automáticamente por GitHub Actions. Homebrew los descarga directamente sin necesitar Rust instalado.

La fórmula no usa `post_install` ni toca `/etc/hosts` ni registra `launchd` por sí sola. Esas operaciones van en `sudo paramo install`.

## Flujo de releases

```bash
# 1. Subir version en Cargo.toml y hacer commit
git add Cargo.toml && git commit -m "chore: bump version to X.Y.Z"

# 2. Crear tag y empujar
git tag -a vX.Y.Z -m "Release vX.Y.Z"
git push origin master
git push origin vX.Y.Z
```

Al empujar el tag, GitHub Actions compila los bottles para arm64 e Intel, crea el GitHub Release, actualiza `Formula/paramo.rb` con los hashes y hace commit automáticamente. No hay pasos manuales en la fórmula.

Resultado para usuarios:

```bash
brew update && brew upgrade paramo
```

## Probar el tap localmente

Para validar que la fórmula instala correctamente desde el tap publicado:

```bash
brew tap attinto/paramo
brew install attinto/paramo/paramo
paramo config show
brew uninstall paramo
```

Para probar la fórmula desde un checkout local sin pasar por GitHub (útil durante desarrollo):

```bash
brew install --build-from-source ./Formula/paramo.rb
```

## Configuración

La configuración activa de sistema vive en:

```text
/etc/paramo/config.toml
```

La plantilla base embebida vive en:

```text
config/default.toml
```

Ejemplo:

```toml
[schedule]
start = 9
end = 18
block_weekends = false

[sites]
list = [
  "tiktok.com",
  "instagram.com",
  "pornhub.com",
  "youtube.com"
]

[hosts]
file = "/etc/hosts"
marker = "# --- PARAMO BLOCK ---"
redirect_ips = ["127.0.0.1", "::1"]

[logging]
file = "/var/log/paramo.log"
level = "info"

[daemon]
interval_seconds = 1200
```

## Troubleshooting

### Una web sigue entrando

PARAMO usa `/etc/hosts`. Algunos navegadores pueden ignorarlo si tienen DNS over HTTPS activo.

En Firefox:

1. Abre `Settings > Privacy & Security`.
2. Busca `DNS over HTTPS`.
3. Déjalo en `Off`.

### `doctor` dice que el servicio no está cargado

Reinstala la parte del sistema:

```bash
sudo paramo install
```

Y vuelve a comprobar:

```bash
paramo doctor
sudo launchctl print system/com.paramo.blocker
```

### Quiero resetear la instalación sin perder la config

```bash
sudo paramo uninstall
sudo paramo install
```
