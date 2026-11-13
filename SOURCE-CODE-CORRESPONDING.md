# Código fuente correspondiente de FoxMulator Next

**Fecha:** 2026-11-13

Este documento identifica el repositorio público, el código fuente y los procedimientos de compilación y empaquetado de FoxMulator Next.

## Repositorio oficial

- **Proyecto:** FoxMulator Next
- **Repositorio:** [https://github.com/DingirAn/FoxMulator-Next](https://github.com/DingirAn/FoxMulator-Next)
- **Rama principal:** `main`
- **Commit público inicial:** `815c006`
- **Licencia del proyecto:** `GPL-3.0-or-later`
- **Texto íntegro de la licencia:** `LICENSE`

Para cada versión publicada, el código fuente correspondiente deberá identificarse mediante un commit o etiqueta Git específica.

## Componentes del código fuente

- `foxmulator-core/`: núcleo de emulación.
- `foxmulator-desktop/`: aplicación de escritorio e interfaz Slint.
- `foxmulator-testrunner/`: herramientas de pruebas.
- `bootrom-foxmulator/`: recursos y fuentes de Boot ROM.
- `resources/`: recursos gráficos y archivos de aplicación.
- `packaging/`: scripts y recursos de empaquetado.
- `Cargo.toml` y `Cargo.lock`: configuración del workspace y dependencias Rust.

Las licencias y avisos de terceros se documentan en `THIRD-PARTY-NOTICES.md` y `docs/licenses/`.

## Compilación Release

La compilación oficial de escritorio para Linux utiliza:

```
cargo build --release -p foxmulator-desktop --features gui,audio,gamepad
```

Las características `gui`, `audio` y `gamepad` deben mantenerse habilitadas.

El directorio de destino efectivo depende de la configuración de Cargo y puede consultarse mediante `cargo metadata`.

## Empaquetado AppImage

Los scripts de empaquetado del proyecto incluyen:

- `packaging/build-appimage.sh`
- `packaging/assemble-appdir.sh`

El artefacto de distribución previsto es:

`FoxMulator-Next-x86_64.AppImage`

El AppImage debe incluir los avisos de licencia correspondientes y permitir identificar el código fuente de la versión distribuida.

## Correspondencia entre binarios y código fuente

Cada Release pública deberá identificar:

1. La versión publicada.
2. El commit o etiqueta Git del código fuente utilizado.
3. Los comandos y scripts necesarios para compilar y empaquetar el programa.
4. Las dependencias y licencias aplicables.

El código fuente publicado debe corresponder a la versión efectivamente utilizada para construir el ejecutable distribuido.

Los archivos generados localmente no se consideran automáticamente equivalentes a una Release pública hasta verificar su correspondencia con el código fuente identificado.

## Disponibilidad del código fuente

El código fuente se encuentra disponible en:

[https://github.com/DingirAn/FoxMulator-Next](https://github.com/DingirAn/FoxMulator-Next)

Las versiones públicas podrán distribuirse mediante GitHub Releases, acompañadas de una referencia inequívoca al código fuente correspondiente y de los materiales necesarios para cumplir las licencias aplicables.
