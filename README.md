# Cual Solitaire

Solitario de escritorio en Rust y egui. Incluye Spider (1, 2 y 4 palos), FreeCell, Klondike y Corazones contra la computadora.

La interfaz está en español. Los mazos gráficos vienen de KDE y se detallan en `COPYRIGHT`.

## Descargar

Las compilaciones publicadas están en [Releases](https://github.com/sstucke/cualsol/releases/latest). Descomprimí el archivo y ejecutá el programa dentro de esa carpeta.

- [Windows (x86_64)](https://github.com/sstucke/cualsol/releases/latest/download/CualSolitaire-windows-x86_64.zip)
- [Linux (x86_64)](https://github.com/sstucke/cualsol/releases/latest/download/CualSolitaire-linux-x86_64.tar.gz)
- [macOS (Apple Silicon)](https://github.com/sstucke/cualsol/releases/latest/download/CualSolitaire-macos-aarch64.tar.gz)

Cada etiqueta `v*` compila las tres plataformas y publica esa release.

## Licencia

[GPL-3.0-or-later](LICENSE). Podés usarlo, modificarlo y compartirlo. Si distribuís una versión modificada, tiene que seguir siendo software libre bajo la misma licencia.

## Compilar

Hace falta Rust estable.

```
cargo run -p cualsol
```

En Windows, desde la carpeta del proyecto:

```
iniciar_windows.cmd
```
