# Undistracted - Bloqueador de Distracciones para macOS (Rust)

Reescritura en Rust del bloqueador de distracciones que modifica `/etc/hosts` para bloquear dominios durante las horas de trabajo.

## Compilación

```bash
cargo build --release
```

El binario compilado estará en `target/release/undistracted`.

## Instalación

```bash
sudo ./target/release/undistracted install
```

Esto:
- Copia el binario a `/usr/local/bin/undistracted`
- Crea `/etc/undistracted/config.toml` con la configuración por defecto
- Copia el `.plist` a `/Library/LaunchDaemons/`
- Configura el LaunchDaemon para ejecutarse cada 20 minutos como root

## Uso

### Ver estado actual (sin sudo)
```bash
./target/release/undistracted status
```

Ejemplo de salida:
```
Viernes 14:23 | 🔴 BLOQUEADO | Horario de bloqueo activo
Próximo cambio: 18:00 (3h 37m desde ahora)
```

### Bloquear/Desbloquear manualmente (requiere sudo)
```bash
sudo ./target/release/undistracted block-now
sudo ./target/release/undistracted unblock-now
```

### Ver configuración
```bash
./target/release/undistracted config show
```

### Desinstalar
```bash
sudo ./target/release/undistracted uninstall
```

## Configuración

Edita `/etc/undistracted/config.toml`:

`config/default.toml` dentro del repo es solo la plantilla embebida para la instalación inicial. Si cambias ese archivo después de instalar, el binario no usará esos cambios hasta que los copies a `/etc/undistracted/config.toml`.

```toml
[schedule]
block_start = 9       # Hora de inicio del bloqueo
block_end = 18        # Hora de fin del bloqueo
block_weekends = false # Bloquear también sábados y domingos

[domains]
list = ["tiktok.com", "instagram.com", ...]

[logging]
file = "/var/log/undistracted.log"
level = "info"
```

## Características principales

✅ **Escritura atómica** de `/etc/hosts` — si el proceso es interrumpido, el archivo no queda corrupto
✅ **Flush DNS automático** tras cada cambio
✅ **Configuración flexible** en TOML
✅ **CLI con subcomandos** — fácil de usar y automatizar
✅ **Tests unitarios** en hosts y scheduler
✅ **Logging estructurado** con rotación de archivos
✅ **Instalación automática** vía el propio binario

## Mejoras respecto a la versión Python

| Feature | Python | Rust |
|---------|--------|------|
| Escritura atómica | ❌ | ✅ |
| Flush DNS automático | ❌ Manual | ✅ Automático |
| Instalación automática | ❌ Manual (launchctl commands) | ✅ `sudo undistracted install` |
| CLI mejorado | ❌ Solo script | ✅ Subcomandos |
| Status sin root | ❌ | ✅ |
| Binario compilado | N/A | ✅ Más difícil de modificar |
| Configuración flexible | ❌ Hardcoded | ✅ TOML editable |

## Logs

Los logs se escriben a `/var/log/undistracted.log` con rotación diaria.

## Troubleshooting

### LaunchDaemon no está activo
Verifica que el plist esté instalado:
```bash
sudo launchctl print system/com.undistracted.blocker
```

### Ver logs de ejecución
```bash
tail -f /var/log/undistracted.log
```

### Firefox sigue mostrando las webs bloqueadas
Firefox puede saltarse `/etc/hosts` si tiene DNS over HTTPS activo:
1. Ve a `Settings > Privacy & Security`
2. Busca `DNS over HTTPS`
3. Ponlo en `Off`

## Arquitectura

```
src/
├── main.rs        # CLI con clap + instalación
├── config.rs      # Configuración con serde+toml
├── hosts.rs       # Manipulación de /etc/hosts
├── scheduler.rs   # Lógica de horario
├── blocker.rs     # Orquestación principal
└── logging.rs     # Setup de logging

config/
└── default.toml   # Configuración por defecto embebida

launchd/
└── com.undistracted.blocker.plist  # Configuración de LaunchDaemon
```

## Testing

```bash
cargo test
```

Tests unitarios en `hosts.rs` y `scheduler.rs` con fechas/tiempos inyectados.
