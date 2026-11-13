# Código fuente correspondiente de FoxMulator Next

Fecha: 2026-11-13

Este documento identifica el código fuente correspondiente al artefacto
Release local. No se realizó ninguna publicación remota.

- Proyecto: árbol fuente de FoxMulator Next correspondiente a esta revisión
- Revisión: `c99e0b9fc65a34fc10a473a2507a124706899e42`
- Rama observada: `fix/adaptive-window-geometry`
- Dependencias Rust: versiones y checksums fijados en `Cargo.lock`
- Target Cargo efectivo: el directorio configurado por Cargo y confirmado con
  `cargo metadata` durante la compilación
- Comando Release:
  `cargo build --release -p foxmulator-desktop --features gui,audio,gamepad`
- Empaquetado: `packaging/build-appimage.sh` y
  `packaging/assemble-appdir.sh`
- Código propio: `foxmulator-core/`, `foxmulator-desktop/`,
  `foxmulator-testrunner/`
- Licencia propia: `GPL-3.0-or-later`, texto íntegro en `LICENSE`

Las modificaciones locales no confirmadas forman parte del árbol de trabajo
observado y deben conservarse junto con esta revisión para reproducir un
binario idéntico. Antes de una distribución pública debe proporcionarse el
árbol fuente correspondiente, sus scripts y `Cargo.lock` de forma accesible
para los receptores, conforme a la GPL aplicable.
