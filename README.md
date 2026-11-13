# FoxMulator Next 🦊

Un emulador nativo de **Game Boy** y **Game Boy Color** escrito en Rust, con
interfaz de escritorio en Slint. La vara tiene núcleo separado, dos consolas
independientes, biblioteca de juegos, guardados persistentes y hasta Cable Link
virtual. No es una maqueta que pinta un cuadrado verde y se da por satisfecha:
el proyecto trae pruebas del núcleo, runners de ROMs y una batería de QA para
revisar que la cosa siga caminando.

Eso sí, mae: es software en desarrollo. Que una ROM cargue, avance y pase una
regresión no significa que cada juego del catálogo completo ya esté validado de
principio a fin.

## ¿Qué es y qué emula?

FoxMulator Next modela una consola Game Boy clásica (DMG) o Game Boy Color
(CGB), según la cabecera de la ROM. El núcleo incluye:

- CPU **SM83**, decodificación de instrucciones, registros, banderas y
  operaciones CB-prefixed.
- MMU de 64 KiB con ROM/RAM bancadas, VRAM y WRAM de CGB, DMA, HDMA y
  restricciones de acceso del bus durante modos de PPU.
- PPU con framebuffer, modos LCD, ventana, sprites, prioridades, paletas y
  bancos de VRAM de CGB.
- APU con los cuatro canales de sonido —dos cuadrados, onda y ruido—,
  secuenciador, mezcla estéreo y generación PCM a 44,1 kHz.
- Temporizador DIV/TIMA/TMA/TAC, interrupciones, joypad y puerto serial.
- Cartuchos ROM-only, MBC1, MBC2, MBC3 con RTC y MBC5, incluyendo SRAM cuando
  la cabecera declara batería.

La ejecución de CGB no es un simple filtro de color: `System::from_rom` elige
el hardware antes de crear CPU, MMU y periféricos.

## Funcionalidades actuales verificadas

- Carga de ROMs `.gb` y `.gbc`, con extensiones sin distinguir mayúsculas.
- Carga nativa de ROMs dentro de `.zip`, directamente en memoria.
- Entradas ROM dentro de subdirectorios del ZIP.
- Validación del encabezado Game Boy, integridad de lectura y límites de tamaño.
- Si un ZIP tiene varias ROMs, se selecciona de forma determinista la primera
  ROM válida en orden lexicográfico, ignorando archivos que no sean ROMs.
- Biblioteca con búsqueda, favoritos, nombres internos, detección DMG/CGB,
  carátulas y edición manual de nombre y portada.
- Caché de carátulas asociada a la identidad lógica de la ROM, también para
  ROMs comprimidas.
- Dos consolas independientes: cada una puede cargar un cartucho distinto
  desde archivo, ZIP o biblioteca.
- Cable Link serial virtual entre las dos consolas, incluyendo reloj interno y
  externo en las pruebas del enlace.
- SRAM persistente (`.sav`) y estados completos (`.state`) con validación de
  identidad de ROM.
- Once ranuras de estado por cartucho: slot 0 para Quick Save y slots 1–10
  manuales, con metadatos `.json`.
- Importación de `.sav` y `.state` desde archivo, migración de nombres antiguos,
  copias de respaldo y escrituras de batería con bloqueo entre procesos.
- Controles de teclado, controles virtuales y gamepad opcional.
- Audio opcional mediante `paplay`/PulseAudio cuando se compila con `audio`.
- Renderer OpenGL/FemtoVG o software, escala entera de 1× a 5×, autoescala,
  cuadrícula LCD, carcasa translúcida, volumen independiente y mute por
  consola.
- Velocidad nativa y boost configurable de 2×, 3× o 4×. El boost cambia el
  ritmo de presentación; no altera artificialmente los ciclos del hardware.
- Español e inglés embebidos, más catálogos XML externos recargables.
- AppImage y bundle Linux reproducible mediante scripts del repositorio.

## Capturas

El repositorio contiene un ícono (`resources/foxmulator.png`), pero no una
captura de pantalla del programa. No voy a disfrazar el ícono de screenshot,
porque diay, tampoco estamos para vender humo.

## Requisitos

El objetivo documentado y probado es **Linux x86_64**.

Para compilar se necesita:

- Rust/Cargo compatible con `rust-version = "1.75"`.
- Herramientas de compilación nativas requeridas por Slint, Winit y Fontconfig.
- Para audio: `paplay` y un servidor PulseAudio disponible. Si `paplay` no
  está disponible, la interfaz puede ejecutarse sin salida audible.
- Para gamepad: compilar con la feature `gamepad` y contar con el soporte del
  sistema que use `gilrs`.

El proyecto no instala paquetes del sistema automáticamente. Los scripts de
empaquetado comprueban dependencias y se detienen si falta alguna.

## AppImage y distribución

Hay un flujo reproducible para construir el AppImage:

```bash
packaging/build-appimage.sh
```

El script compila con `gui,audio,gamepad`, prepara un `AppDir` sin ROMs ni
partidas y usa `linuxdeploy` y `appimagetool` para generar:

```text
FoxMulator-Next-x86_64.AppImage
```

Esas dos herramientas deben estar disponibles en `PATH`, junto con `cargo`,
`convert` y `ldd`. El script no descarga ni instala dependencias.

También existe un bundle `.tar.gz` reproducible:

```bash
scripts/package-release.sh
```

Ese bundle contiene el ejecutable, `LICENSE`, `README.md` y `SHA256SUMS`. No
copia ROMs, partidas ni estados.

## Compilar desde el código fuente

Desde la raíz del repositorio:

```bash
# Núcleo sin interfaz
cargo build -p foxmulator-core

# Aplicación de escritorio completa
cargo build -p foxmulator-desktop --features gui,audio,gamepad

# Compilar en modo release
cargo build -p foxmulator-desktop --release --features gui,audio,gamepad
```

La feature `gui` habilita Slint y los diálogos de archivo. `audio` habilita el
productor PCM de escritorio y `gamepad` añade la entrada mediante `gilrs`.

## Uso básico

Ejecutar con una ROM:

```bash
cargo run -p foxmulator-desktop --features gui,audio,gamepad -- \
  'mi-juego.gb'
```

También se puede abrir una ROM CGB o un ZIP:

```bash
cargo run -p foxmulator-desktop --features gui,audio,gamepad -- \
  'mi-juego.gbc'

cargo run -p foxmulator-desktop --features gui,audio,gamepad -- \
  'mi-juego.zip'
```

Para arrancar con dos cartuchos independientes:

```bash
cargo run -p foxmulator-desktop --features gui,audio,gamepad -- \
  'primero.gb' --rom2 'segundo.gbc'
```

Opciones útiles:

```bash
cargo run -p foxmulator-desktop --features gui,audio,gamepad -- --help
cargo run -p foxmulator-desktop --features gui,audio,gamepad -- --version
cargo run -p foxmulator-desktop --features gui,audio,gamepad -- --fullscreen
cargo run -p foxmulator-desktop --features gui,audio,gamepad -- --mute
cargo run -p foxmulator-desktop --features gui,audio,gamepad -- --link \
  'primero.gb' --rom2 'segundo.gb'
```

`--link` requiere dos ROMs válidas. `--rom2` carga la segunda consola, pero no
la conecta automáticamente: el Cable Link se activa desde Opciones o con
`--link`.

La ayuda integrada (`--help`) muestra las opciones disponibles y sus errores de
validación; `--rom2` carga la segunda consola y `--link` activa el enlace serial
virtual cuando se proporcionan dos ROMs válidas.

## Controles

Los valores predeterminados son:

| Acción | Consola 1 | Consola 2 |
|---|---|---|
| Direcciones | Flechas | J / L / I / K |
| A | X | U |
| B | Z | O |
| Select | Backspace | P |
| Start | Enter | ; |
| Quick Save | F5 | F9 |
| Quick Load | F6 | F10 |

Los controles se pueden reasignar desde la interfaz. También se pueden usar
controles virtuales y gamepads cuando la aplicación fue compilada con la feature
correspondiente.

## Biblioteca y formatos compatibles

La biblioteca se configura seleccionando una carpeta desde la interfaz. Una
carpeta vacía sigue siendo válida; luego el escaneo decide si encuentra juegos.
Se reconocen:

- Archivos directos `.gb` y `.gbc`.
- Archivos `.zip` con una o más entradas `.gb`/`.gbc`.
- Entradas ROM ubicadas en subdirectorios internos del ZIP.

Las ROMs se validan antes de mostrarse. El ZIP no se extrae de forma permanente:
la entrada seleccionada se lee a memoria durante la carga. Los archivos ZIP
corruptos, sin ROM válida, incompatibles o que excedan los límites se ignoran
en el escaneo o producen un error de carga; no se tratan como un juego genérico.

Las carátulas se buscan mediante el índice de CRC y el catálogo de miniaturas
configurado por la aplicación. Se almacenan en caché por identidad de ROM. La
descarga es opcional y ocurre fuera del ciclo de emulación; sin red, siguen
funcionando las carátulas ya guardadas y las manuales.

## Guardados y Save States

Los `.sav` contienen la RAM persistente del cartucho y, cuando corresponde,
los datos del RTC de MBC3. Los `.state` contienen una instantánea completa del
sistema: CPU, MMU/MBC, memoria, PPU, APU, temporizadores, joypad y serial.

Los guardados se basan en el archivo físico original:

- Para una ROM directa, quedan junto a la ROM.
- Para una ROM dentro de ZIP, quedan junto al ZIP original.
- Nunca se escribe dentro del ZIP ni en una carpeta temporal de extracción.
- La entrada interna del ZIP participa en la identidad lógica.
- Los nombres ZIP incluyen un identificador adicional de entrada para evitar
  colisiones con una ROM directa del mismo nombre o con otra entrada.

Los estados usan once slots y un `.json` con identidad, título, fecha, tamaño y
formato. La carga valida la ROM antes de reemplazar el estado activo. Las
partidas antiguas se migran solo cuando son válidas y no ambiguas; las fuentes
originales se conservan como respaldo.

La identidad lógica de la ROM, la validación del estado y las reglas de
ubicación anteriores son parte del flujo de persistencia del escritorio.

## Dos consolas y Cable Link

Cada consola tiene su propio `System`, cartucho, SRAM, estado, controles y
framebuffer. Se puede cargar o expulsar una sin derivar la otra. El enlace
serial virtual intercambia bytes entre ambos puertos y genera sus interrupciones
correspondientes.

La conexión virtual está probada con relojes internos y externos. Eso no equivale
a validar hardware físico ni una sesión completa de juego con dos personas; esa
parte sigue siendo trabajo manual.

## Arquitectura

El workspace tiene tres crates:

```text
foxmulator-core/
  CPU SM83, MMU, cartuchos/MBC, PPU, APU, timer, interrupciones,
  joypad, serial y System.

foxmulator-desktop/
  binario foxmulator, Slint, entrada, renderizado, audio, biblioteca,
  RomSource, SRAM y Save States.

foxmulator-testrunner/
  blargg-runner, commercial-runner, audio-benchmark,
  apu-diagnostic y qa-runner.
```

La carga ZIP vive en `foxmulator-desktop/src/rom_source.rs`; no cambia CPU,
PPU, APU, MMU, Cable Link ni el bucle de emulación. Tanto una ROM directa como
una entrada ZIP alimentan el mismo `System`.

La interfaz se define en `foxmulator-desktop/src/Ui.slint` y la lógica de
interacción en `main.rs`. Slint ofrece renderer software y FemtoVG/OpenGL; la
preferencia se guarda y se aplica al siguiente inicio. El directorio de caché y
las preferencias dependen de las rutas de configuración/cache de la plataforma.

## Pruebas y validación

Pruebas unitarias e integración:

```bash
cargo test --workspace --all-targets
```

Calidad y build estricto:

```bash
cargo fmt --all -- --check
git diff --check
cargo clippy --workspace --all-targets -- -D warnings
cargo build -p foxmulator-desktop --release --features gui,audio,gamepad
```

Las pruebas del workspace cubren el núcleo y los componentes que tengan pruebas
automatizadas. La validación automática de una ROM confirma únicamente la
carga, el avance, el protocolo o los frames que mida la prueba; un timeout o una
prueba omitida no equivale a PASS. Estas comprobaciones tampoco pretenden
probar jugabilidad completa, tearing del monitor, escucha humana del audio o
gamepads físicos.

## Limitaciones conocidas y trabajo pendiente

- La compatibilidad de hardware todavía no es una certificación de todos los
  juegos ni de todos los accesorios.
- Hay diferencias conocidas en precisión de OAM DMA/POP y otros bordes de
  timing. La suite OAM no está completamente verde y no se oculta ese dato.
- La validación automática de ROMs confirma carga, avance, protocolo o frames
  según el runner; no demuestra una partida completa ni compatibilidad total.
- El Cable Link validado es virtual. No hay validación declarada con dos
  consolas físicas ni dos mandos físicos.
- El selector de ZIP usa una política determinista para múltiples ROMs; no hay
  todavía un selector de entradas internas dentro del archivo.
- La salida de audio depende de `paplay`/PulseAudio y la sincronización visual
  OpenGL depende también del driver, X11/Wayland y el compositor.
- Desktop y TestRunner ejecutan automáticamente las Boot ROMs libres SameBoot
  integradas para DMG/CGB; no se admiten BIOS externas ni rutas de firmware.

Estas limitaciones no son una lista de promesas futuras disfrazadas. Son,
simplemente, las varas que todavía requieren más brete.

## Contribuciones

Pull requests y reportes de errores son bienvenidos. Para que el brete sea
revisable:

1. Explicá el problema o comportamiento reproducible.
2. Mantené los cambios acotados al subsistema correspondiente.
3. Añadí o actualizá pruebas cuando el cambio sea verificable.
4. Ejecutá formato, tests, Clippy y el QA relevante.
5. No incluyás ROMs, partidas, Save States, credenciales ni recursos de
   terceros sin verificar sus permisos.

Los archivos locales de ROMs y partidas están pensados para permanecer fuera de
Git. Revisá `AGENTS.md` antes de tocar el núcleo o los flujos de persistencia.

## Licencia y créditos

Los `Cargo.toml` declaran `GPL-3.0-or-later` para el workspace y sus tres crates.
El archivo [`LICENSE`](LICENSE) contiene el texto de la **GNU General Public
License, versión 3**. El proyecto no reclama propiedad sobre ROMs, carátulas,
miniaturas ni bases de datos de terceros; esos recursos tienen que respetarse
según sus propias condiciones. El bundle de release no copia ROMs ni partidas.

La interfaz usa [Slint](https://slint.dev/), la compresión ZIP usa la crate
[`zip`](https://crates.io/crates/zip), y el empaquetado Linux se apoya en
`linuxdeploy`/`appimagetool`. El resto de dependencias está declarado en los
`Cargo.toml`; Cargo.lock fija las versiones resueltas.

Si algo de este README contradice el comportamiento observable del código,
mandan el código y las pruebas. Pura vida.
