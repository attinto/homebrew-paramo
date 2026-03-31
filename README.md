# PARAMO

PARAMO es un bloqueador de distracciones para macOS escrito en Rust. Gestiona una sección propia dentro de `/etc/hosts`, puede ejecutarse manualmente desde CLI o TUI, y puede quedar instalado como daemon de `launchd` para aplicar el bloqueo por horario sin intervención manual.

## Estado del proyecto

El repositorio ya cubre el flujo local completo:

- CLI para consultar estado, gestionar sitios, cambiar horario e idioma.
- TUI interactiva al ejecutar `paramo` sin argumentos.
- Instalación del sistema con `sudo paramo install`.
- Diagnóstico real con comprobaciones sobre configuración, `launchd`, `/etc/hosts` y restos de `undistracted`.
- Migración desde una instalación anterior de `undistracted`.

El empaquetado de Homebrew no está incluido todavía.

## Requisitos

- macOS
- Rust toolchain estable
- permisos de administrador para `install`, `uninstall`, `block`, `unblock`, `site add/remove` y `schedule set`

## Compilar

```bash
cargo build --release
```

El binario queda en:

```text
target/release/paramo
```

## Flujo recomendado de prueba local

### 1. Comprobar que el proyecto está sano

```bash
cargo test
cargo clippy --all-targets -- -D warnings
```

### 2. Ver el estado actual sin instalar nada

```bash
cargo run -- status
cargo run -- doctor
```

Si todavía no existe `/etc/paramo/config.toml`, PARAMO lo indicará y usará:

- la configuración legacy de `/etc/undistracted/config.toml`, si existe
- o la plantilla embebida de `config/default.toml`

### 3. Instalar la parte del sistema

```bash
sudo ./target/release/paramo install
```

Esto hace lo siguiente:

- crea o normaliza `/etc/paramo/config.toml`
- migra `/etc/undistracted/config.toml` si hace falta
- limpia el daemon legacy de `undistracted` si sigue presente
- sincroniza el estado actual en `/etc/hosts`
- genera y registra `/Library/LaunchDaemons/com.paramo.blocker.plist`
- valida que `launchd` haya cargado `com.paramo.blocker`

### 4. Verificar la instalación

```bash
paramo doctor
paramo status
sudo launchctl print system/com.paramo.blocker
```

Deberías ver:

- configuración activa en `/etc/paramo/config.toml`
- plist válido
- servicio cargado en `launchd`
- bloque de hosts sincronizado

## Uso rápido

### Abrir la TUI

```bash
paramo
```

Si el terminal es interactivo, abre la TUI. Si no lo es, muestra el estado actual.

### Ver estado

```bash
paramo status
```

### Bloquear y desbloquear manualmente

```bash
sudo paramo block
sudo paramo unblock
```

### Gestionar sitios

```bash
paramo site list
sudo paramo site add youtube.com
sudo paramo site remove youtube.com
```

PARAMO normaliza dominios y evita duplicados como `youtube.com` y `www.youtube.com`.

### Ver y cambiar el horario

```bash
paramo schedule show
sudo paramo schedule set --start 9 --end 18 --weekends off
sudo paramo schedule set --start 22 --end 8 --weekends on
```

Se soportan franjas que cruzan medianoche.

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

Muestra:

- `/etc/paramo/config.toml` si existe
- `/etc/undistracted/config.toml` si todavía estás en modo legacy
- o la plantilla embebida si aún no has instalado PARAMO

### Diagnóstico

```bash
paramo doctor
```

`doctor` revisa al menos:

- de dónde sale la configuración efectiva
- si quedan restos de `undistracted`
- si el plist de `launchd` existe y coincide con la configuración
- si el servicio está realmente cargado en `launchd`
- si la lista de sitios está vacía
- si el bloque gestionado en `/etc/hosts` está duplicado o desincronizado
- si conviene revisar DNS over HTTPS en el navegador

## Instalación y desinstalación

### Instalar

```bash
sudo paramo install
```

### Desinstalar

```bash
sudo paramo uninstall
```

`uninstall` hace lo siguiente:

- descarga y elimina el daemon de PARAMO
- retira el bloque gestionado de `/etc/hosts`
- elimina el binario manual si estaba en `/usr/local/bin/paramo`
- conserva `/etc/paramo/config.toml`

## Configuración

La configuración activa vive en:

```text
/etc/paramo/config.toml
```

La plantilla base del repositorio vive en:

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

## TUI

Atajos principales:

- `Tab` y `Shift+Tab` cambian de pestaña
- `q` sale
- `b` bloquea ahora
- `u` desbloquea ahora
- `r` refresca el estado
- en `Sitios`: `a` añade y `d` elimina
- en `Horario`: `↑` y `↓` seleccionan campo, `←` y `→` cambian valor
- en `Ajustes`: `←` y `→` cambian el idioma
- en `Diagnóstico`: `g` relanza las comprobaciones

Si la TUI se abre sin `sudo`, las acciones que escriben en el sistema quedan en modo lectura.

## Estructura del proyecto

```text
src/
├── main.rs         # CLI y entrada principal
├── tui.rs          # Interfaz de terminal
├── config.rs       # Configuración del sistema y normalización
├── preferences.rs  # Preferencias del usuario
├── i18n.rs         # Textos ES/EN
├── blocker.rs      # Sincronización de bloqueo con /etc/hosts
├── hosts.rs        # Lectura/escritura del bloque gestionado
├── scheduler.rs    # Reglas de horario y próximo cambio
├── doctor.rs       # Diagnóstico de instalación y estado
├── install.rs      # Instalación, migración y uninstall
├── launchd.rs      # Template y utilidades de launchd
├── logging.rs      # Logging con fallback limpio
└── paths.rs        # Rutas del sistema y del usuario

config/
└── default.toml    # Plantilla embebida de configuración

launchd/
└── com.paramo.blocker.plist   # Template embebido del daemon
```

## Troubleshooting

### Una web sigue entrando

PARAMO usa `/etc/hosts`. Algunos navegadores pueden ignorarlo si tienen DNS over HTTPS activo.

En Firefox:

1. Abre `Settings > Privacy & Security`
2. Busca `DNS over HTTPS`
3. Déjalo en `Off`

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

### Quiero resetear la instalación sin perder mi config

```bash
sudo paramo uninstall
sudo paramo install
```
