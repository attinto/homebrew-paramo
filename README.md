# PARAMO

PARAMO es un bloqueador de distracciones para macOS escrito en Rust. Gestiona una sección propia dentro de `/etc/hosts`, ofrece CLI y TUI, y puede quedar instalado como daemon de `launchd` para aplicar el bloqueo por horario sin tener que abrir la app manualmente. 


## Instalación con Homebrew

El objetivo de este repo es funcionar como un tap de Homebrew de un solo repositorio.

La experiencia final esperada es:

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

Homebrew instala solo el binario CLI. La integración con `/etc/hosts` y `launchd` sigue siendo un paso explícito aparte:

```bash
sudo paramo install
```

Después de eso, las acciones diarias del CLI y de la TUI funcionan sin `sudo`, delegando en el daemon instalado.

## Nombre del repositorio

Para que `brew tap attinto/paramo` funcione con la convención estándar de Homebrew, el repositorio en GitHub debe llamarse:

```text
homebrew-paramo
```

Este árbol ya está preparado para ese nombre. Si todavía no has renombrado el repo en GitHub, hazlo antes de publicar el tap.

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

La fórmula vive en:

```text
Formula/paramo.rb
```

En v1 instala compilando desde source con Rust. No usa `post_install`, no toca `/etc/hosts` ni registra `launchd` por sí sola; esas operaciones siguen en `sudo paramo install`.

## Flujo de releases

Cada release publicada para Homebrew debe seguir este orden:

1. Subir `version` en `Cargo.toml`.
2. Hacer commit de release.
3. Crear el tag Git `vX.Y.Z`.
4. Empujar el commit y el tag al repo `attinto/homebrew-paramo`.
5. Actualizar `Formula/paramo.rb` para que `tag` y `revision` apunten a esa release.
6. Hacer commit de la fórmula actualizada en `main`.
7. Empujar `main`.

Resultado para usuarios:

- `brew update` actualiza el tap.
- `brew upgrade paramo` instala la nueva versión declarada en la fórmula.

## Probar el tap localmente

Una vez el repositorio exista en GitHub con el nombre definitivo y el tag de la versión esté publicado, puedes validar el flujo así:

```bash
brew tap attinto/paramo
brew install --build-from-source attinto/paramo/paramo
paramo config show
brew uninstall paramo
```

Después de ese primer tag, también puedes instalar la fórmula directamente desde este checkout:

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
