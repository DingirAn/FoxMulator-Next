# FoxMulator Next — estructura pública

Fecha de revisión: 2026-11-13

Este archivo describe únicamente la estructura que forma parte del proyecto
público. Las fuentes, los manifiestos y `Cargo.lock` son la referencia del
comportamiento observable; no se deben incluir ROMs, partidas, estados,
credenciales ni artefactos privados.

## Workspace

- `foxmulator-core/`: núcleo sin interfaz; CPU SM83, MMU y cartuchos/MBC, PPU,
  APU, temporizador, joypad, serial y `System`.
- `foxmulator-desktop/`: aplicación de escritorio Slint/winit, entrada,
  renderizado, audio, biblioteca, carga de ROMs, SRAM y estados.
- `foxmulator-testrunner/`: ejecutables headless y herramientas de prueba del
  núcleo y del escritorio.

## Recursos y distribución

- `foxmulator-core/resources/bootroms/`: Boot ROMs libres integradas y sus
  avisos de licencia.
- `resources/`: iconos y recursos visuales públicos usados por la aplicación.
- `packaging/`: ensamblaje del AppDir y empaquetado Linux.
- `docs/licenses/`: textos de licencia de terceros distribuidos con el proyecto.
- `LICENSE`: GNU GPL versión 3 o posterior para el código original de
  FoxMulator.
- `THIRD-PARTY-NOTICES.md`: inventario de componentes y avisos de terceros.
- `SOURCE-CODE-CORRESPONDING.md`: identificación del código fuente
  correspondiente a los binarios locales.

## Convenciones públicas

1. Mantener separados el núcleo, la interfaz y las herramientas headless.
2. Preservar las licencias y avisos de cada componente; no relicenciar
   materiales de terceros.
3. No modificar ROMs, Boot ROMs o recursos de terceros para fabricar resultados.
4. Mantener los cambios acotados y documentar las limitaciones observables.
5. Antes de publicar cambios, revisar el diff y ejecutar las comprobaciones
   apropiadas para los archivos afectados.

La configuración local de Cargo, los directorios temporales, los binarios
generados y las ROMs privadas no forman parte de esta estructura pública.
