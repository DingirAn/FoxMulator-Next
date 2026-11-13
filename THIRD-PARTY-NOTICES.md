# FoxMulator Next — avisos de terceros

Fecha de revisión: 2026-11-13

El código original de FoxMulator Next está bajo `GPL-3.0-or-later`; este
documento no relicencia los componentes de terceros. Las versiones exactas
del grafo completo están fijadas en `Cargo.lock`. Las expresiones SPDX de este
inventario provienen de `cargo metadata` para ese grafo.

## Interfaz y ventana

- **Slint 1.18.1** (`slint`, `slint-build` y crates `i-slint-*` usados por la
  configuración Release): `GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0
  OR LicenseRef-Slint-Software-3.0`. Para esta distribución se selecciona la
  modalidad GPLv3 (`GPL-3.0-only`). Los textos de las tres alternativas se
  incluyen en `docs/licenses/slint/` y en el AppImage.
- **winit 0.30.13**: `MIT OR Apache-2.0`.
- **rfd 0.15.4**: `MIT OR Apache-2.0`.
- **gilrs 0.10.10** y `gilrs-core 0.5.15`: licencias declaradas por Cargo;
  sus expresiones y versiones aparecen en el inventario completo generado
  desde `Cargo.lock`.

## Dependencias Rust directas

| Componente | Versión fijada | SPDX declarado |
|---|---:|---|
| `bytemuck` | 1.25.2 | `Zlib OR Apache-2.0 OR MIT` |
| `byteorder` | 1.5.0 | `Unlicense OR MIT` |
| `clap` | 4.6.7 | `MIT OR Apache-2.0` |
| `directories` | 5.0.1 | `MIT OR Apache-2.0` |
| `quick-xml` | 0.41.0 | `MIT` |
| `rand` | 0.8.8 | `MIT OR Apache-2.0` |
| `reqwest` | 0.12.28 | `MIT OR Apache-2.0` |
| `serde` / `serde_json` | 1.0.229 / 1.0.154 | `MIT OR Apache-2.0` |
| `zip` | 0.6.6 | `MIT` |

Las dependencias transitivas del ejecutable y de `slint-build` conservan sus
licencias originales. El inventario resoluble y reproducible es el conjunto
de paquetes de `Cargo.lock` y sus campos `license` publicados en crates.io;
no se copió código fuente de esas dependencias al AppImage.

## Boot ROMs y recursos externos

- **SameBoy/SameBoot**: licencia Expat/MIT. Aviso completo en
  `foxmulator-core/resources/bootroms/SAMEBOY-LICENSE.txt`.
- Proyecto de procedencia: <https://github.com/LIJI32/SameBoy>.
- Commit fuente de SameBoy: `ea667ad79aa28409f034b1fd6da7304e1844750d`.
- La DMG usa el loader libre original de SameBoy. El binario de trabajo se
  genera desde `bootrom-branding/src/dmg_boot.asm` y `dmg_sameboot.inc`, y se
  conserva como `foxmulator-core/resources/bootroms/dmg_boot.bin` (256 bytes,
  SHA-256 `6f64da4cecd7e54e2f928eb3e3ba7810a7a567d0d247cc71737d1771e073a916`).
- La CGB usa la variante especializada de FoxMulator, que arranca hardware CGB
  y no contiene una ruta de compatibilidad DMG. Se genera desde
  `bootrom-foxmulator/src/cgb_boot.asm`, `hardware.inc` y
  `assets/logo.png`, mediante los recursos de RGBDS, y se conserva como
  `foxmulator-core/resources/bootroms/cgb_boot.bin` (2304 bytes, SHA-256
  `000329c674db758218d9e04a46ce8a390e8b89e66d33a14f6415ca77f666fb6c`).
- Los binarios se incorporan al paquete mediante `include_bytes!`; la selección
  se realiza según el modelo de la ROM y no requiere lecturas de disco durante
  la emulación. No se admiten BIOS externas ni firmware propietario.
- RGBDS de referencia: commit `f7b8207322aa9e3714ef0d98d01989f320103026`.
- **`hardware.inc` de gbdev**: CC0-1.0. El aviso se conserva en
  `foxmulator-core/resources/bootroms/HARDWARE-INC-CC0.txt`.
- El branding de FoxMulator y sus atribuciones pertenecen a los recursos de
  `bootrom-branding`; no se relicencian como SameBoy ni como código original de
  FoxMulator.

## Bibliotecas Linux empaquetadas

`linuxdeploy` incluye las bibliotecas dinámicas requeridas por la compilación
Linux. Sus archivos `copyright` se conservan bajo `usr/share/doc/` dentro del
AppImage. No se afirma una licencia única para ese conjunto: cada aviso
pertenece a su biblioteca correspondiente.

## Código fuente correspondiente

La correspondencia exacta del binario local se documenta en
`SOURCE-CODE-CORRESPONDING.md`. Este artefacto no constituye por sí mismo una
publicación remota ni una oferta pública de código fuente.
