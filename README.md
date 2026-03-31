# PARAMO

PARAMO es un bloqueador de distracciones para macOS escrito en Rust. Modifica `/etc/hosts`, se integra con `launchd`, ofrece una CLI clara y abre una TUI interactiva al ejecutar `paramo` sin argumentos.

## Qué hace

- Bloquea dominios por horario.
- Permite bloquear y desbloquear manualmente.
- Gestiona sitios desde CLI y desde la TUI.
- Guarda el idioma del usuario (`es` por defecto, `en` opcional).
- Migra una instalación anterior de `undistracted` cuando existe.
- Se puede distribuir con Homebrew tap.

## Compilación

```bash
cargo build --release
```

El binario compilado queda en `target/release/paramo`.

## Instalación manual

```bash
sudo ./target/release/paramo install
```

Esto hace lo siguiente:

- crea `/etc/paramo/config.toml` si no existe
- migra `/etc/undistracted/config.toml` si detecta una instalación anterior
- registra `com.paramo.blocker` en `launchd`
- usa el binario actual para el daemon

## Instalación con Homebrew tap

La idea de distribución es:

```bash
brew tap attinto/paramo
brew install attinto/paramo/paramo
```

o directamente:

```bash
brew install attinto/paramo/paramo
```

Después de instalar con Homebrew, hay que registrar el daemon una vez:

```bash
sudo paramo install
```

Homebrew instala el binario; `paramo install` prepara la parte del sistema (`/etc/paramo` y `launchd`).

## Uso rápido

### Abrir la TUI

```bash
paramo
```

Si el terminal es interactivo, `paramo` abre la TUI con el panel principal, el ASCII art de PARAMO y acceso a:

- Inicio
- Sitios
- Horario
- Ajustes
- Diagnóstico

### Estado actual

```bash
paramo status
```

### Bloquear y desbloquear manualmente

```bash
sudo paramo block
sudo paramo unblock
```

### Listar, añadir y quitar sitios

```bash
paramo site list
sudo paramo site add youtube.com
sudo paramo site remove youtube.com
```

### Ver y cambiar el horario

```bash
paramo schedule show
sudo paramo schedule set --start 9 --end 18 --weekends off
```

### Cambiar el idioma

```bash
paramo lang show
paramo lang set en
paramo lang set es
```

### Diagnóstico

```bash
paramo doctor
```

### Ver la configuración activa

```bash
paramo config show
```

### Desinstalar

```bash
sudo paramo uninstall
```

`paramo uninstall` elimina el daemon y la instalación manual del binario si existe, pero conserva `/etc/paramo/config.toml`.

## Configuración

La configuración activa vive en:

```text
/etc/paramo/config.toml
```

Plantilla por defecto:

```toml
[schedule]
start = 9
end = 18
block_weekends = false

[sites]
list = ["youtube.com", "instagram.com"]

[logging]
file = "/var/log/paramo.log"
level = "info"

[daemon]
interval_seconds = 1200
```

`config/default.toml` en el repo es la plantilla embebida. Una vez instalado, la fuente de verdad es `/etc/paramo/config.toml`.

## Migración desde Undistracted

Si existe una instalación anterior, PARAMO:

- lee `/etc/undistracted/config.toml`
- puede reutilizar la lista de sitios y el horario
- avisa desde `paramo doctor`
- elimina el daemon antiguo al ejecutar `sudo paramo install`

## TUI

Atajos principales dentro de la TUI:

- `Tab` y `Shift+Tab` cambian de pestaña
- `q` sale
- `b` bloquea ahora
- `u` desbloquea ahora
- `r` refresca el estado
- en `Sitios`: `a` añade y `d` elimina
- en `Horario`: cursores cambian los campos
- en `Ajustes`: izquierda/derecha cambia el idioma
- en `Diagnóstico`: `g` relanza las comprobaciones

Si abres `paramo` sin `sudo`, la TUI funciona en modo lectura para las acciones que escriben en `/etc/hosts` o `/etc/paramo/config.toml`.

## Diagnóstico

`paramo doctor` revisa al menos:

- si existe la configuración activa
- si hay restos de `undistracted`
- si el daemon de `launchd` está instalado
- si hay sitios configurados
- si el bloque en `/etc/hosts` está duplicado o desincronizado
- si conviene revisar DNS over HTTPS en el navegador

## Estructura

```text
src/
├── main.rs         # CLI principal y entrada de la app
├── tui.rs          # Interfaz interactiva de terminal
├── config.rs       # Configuración del sistema y mutaciones de sitios/horario
├── preferences.rs  # Preferencias del usuario (idioma)
├── i18n.rs         # Textos ES/EN
├── blocker.rs      # Sincronización del bloqueo con /etc/hosts
├── hosts.rs        # Lectura/escritura y bloque gestionado
├── scheduler.rs    # Horario y próximo cambio
├── doctor.rs       # Diagnóstico de instalación y estado
├── install.rs      # Instalación, migración y launchd
├── logging.rs      # Logging con fallback limpio a stderr
└── paths.rs        # Rutas del sistema y del usuario

config/
└── default.toml

launchd/
└── com.paramo.blocker.plist
```

## Testing

```bash
cargo test
```

## Troubleshooting

### Una web sigue entrando

PARAMO usa `/etc/hosts`. Algunos navegadores pueden saltárselo si tienen DNS over HTTPS activo.

En Firefox:

1. `Settings > Privacy & Security`
2. buscar `DNS over HTTPS`
3. dejarlo en `Off`

### El daemon no está activo

```bash
sudo launchctl print system/com.paramo.blocker
```

### Quiero reinstalar la parte del sistema

```bash
sudo paramo install
```
