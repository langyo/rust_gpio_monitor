# GPIO Monitor

![Crates.io License](https://img.shields.io/crates/l/gpio_monitor)
![Crates.io Version](https://img.shields.io/crates/v/gpio_monitor)
![GitHub Actions Workflow Status](https://img.shields.io/github/actions/workflow/status/langyo/rust_gpio_monitor/test.yml)

## Introduction

A sysfs-based GPIO status indicator for aiding embedded development and pinout identification.

> Only available on **Linux**.

## Install

You can install `gpio_monitor` using `cargo`:

```bash
cargo install gpio_monitor
```

## Usage

You can run `gpio_monitor` with the following command:

```bash
gpio_monitor [OPTIONS]
```

## Options

```text
Usage: gpio_monitor [OPTIONS]
Options:
  -h, --help      Print help
  -V, --version   Print version
  -C, --count <COUNT>
                  Number of GPIOs to monitor (default: 256)
```
