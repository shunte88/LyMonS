#!/bin/sh
# LyMonS bus discovery helper.
#
# Lists the I2C buses, SPI nodes and GPIO controllers this board exposes, so
# you can fill in the display.bus block of lymons.yaml without guessing.
#
# Shipped inside every LyMonS package and installed alongside the runtime
# assets. It is read-only: it inspects /dev and /sys and prints, nothing else.
#
# The board family is detected from the device tree, because the three things
# that differ between boards are the bus numbers, the way you enable a bus and
# the way GPIO lines are numbered.

FAMILY="unknown"
MODEL="unknown"
IS_PCP="no"

if [ -r /proc/device-tree/model ]; then
    MODEL=$(tr -d '\000' < /proc/device-tree/model)
elif [ -r /sys/firmware/devicetree/base/model ]; then
    MODEL=$(tr -d '\000' < /sys/firmware/devicetree/base/model)
fi

COMPAT=""
if [ -r /proc/device-tree/compatible ]; then
    COMPAT=$(tr '\000' ' ' < /proc/device-tree/compatible)
fi

case "${MODEL} ${COMPAT}" in
    *[Rr]aspberry*|*brcm,bcm*)  FAMILY="pi" ;;
    *rockchip*|*Rockchip*)      FAMILY="rockchip" ;;
    *allwinner*|*Allwinner*)    FAMILY="allwinner" ;;
esac

# piCorePlayer runs TinyCore, where packages and bus setup work differently.
if command -v pcp > /dev/null 2>&1 || \
   grep -qi "tinycore\|picore" /etc/issue 2>/dev/null; then
    IS_PCP="yes"
fi

# Package install hint for the tools this script would like to have.
pkg_hint() {
    if [ "${IS_PCP}" = "yes" ]; then
        echo "tce-load -wi $1"
    else
        echo "sudo apt install $1"
    fi
}

# How to turn a bus on for this board.
enable_hint() {
    if [ "${FAMILY}" = "pi" ] && [ "${IS_PCP}" = "yes" ]; then
        echo "piCorePlayer web interface, Tweaks page, then reboot"
    elif [ "${FAMILY}" = "pi" ]; then
        echo "sudo raspi-config, Interface Options, then reboot"
    elif [ "${FAMILY}" = "rockchip" ] || [ "${FAMILY}" = "allwinner" ]; then
        echo "add an overlay to /boot/armbianEnv.txt, then reboot"
    else
        echo "enable the bus in your board's device tree or overlay config"
    fi
}

echo "Board"
echo "-----"
echo "  Model    : ${MODEL}"
echo "  Family   : ${FAMILY}"
[ "${IS_PCP}" = "yes" ] && echo "  Distro   : piCorePlayer (TinyCore)"
echo ""

echo "I2C buses  (display.bus.bus when type: i2c)"
echo "---------"
if ls /dev/i2c-* > /dev/null 2>&1; then
    ls -1 /dev/i2c-*
else
    echo "  none found, $(enable_hint)"
fi
echo ""

echo "SPI nodes  (display.bus.bus when type: spi)"
echo "---------"
if ls /dev/spidev* > /dev/null 2>&1; then
    ls -1 /dev/spidev*
else
    echo "  none found, $(enable_hint)"
fi
echo ""

echo "GPIO controllers  (display.bus.gpio_chip, SPI panels only)"
echo "----------------"
if command -v gpiodetect > /dev/null 2>&1; then
    gpiodetect | sed 's/^/  /'
else
    FOUND=0
    for d in /sys/bus/gpio/devices/gpiochip* /sys/class/gpio/gpiochip*; do
        [ -e "$d" ] || continue
        FOUND=1
        printf '  %-12s label=%s lines=%s\n' \
            "$(basename "$d")" \
            "$(cat "$d/label" 2>/dev/null || echo '?')" \
            "$(cat "$d/ngpio" 2>/dev/null || echo '?')"
    done
    [ "${FOUND}" = 0 ] && echo "  none found, check you are in the 'gpio' group"
    echo "  (for reliable output: $(pkg_hint gpiod))"
fi
echo ""

echo "GPIO line numbering"
echo "-------------------"
case "${FAMILY}" in
    pi)
        echo "  The whole 40-pin header is one controller and its line offsets"
        echo "  are the BCM numbers, so dc_pin: 24 means BCM 24."
        echo ""
        echo "  Leave gpio_chip unset. LyMonS finds the header controller by"
        echo "  label (pinctrl-rp1 on a Pi 5, pinctrl-bcm2711 on a Pi 4,"
        echo "  pinctrl-bcm2835 on earlier boards), which matters because the"
        echo "  header is not always gpiochip0. Set gpio_chip only to override."
        ;;
    rockchip)
        echo "  One controller per bank, gpio0 through gpio4, 32 lines each."
        echo "  Pins are bank-relative, NOT BCM numbers."
        echo ""
        echo "    line = group * 8 + index        (group A=0 B=1 C=2 D=3)"
        echo "    PC7  = 2 * 8 + 7 = 23           on bank 3"
        echo "    -> gpio_chip: gpio3, dc_pin: 23"
        ;;
    allwinner)
        echo "  Two controllers: the main pinctrl and the R_PIO carrying PL."
        echo "  Pins are flat within each controller, NOT BCM numbers."
        echo ""
        echo "    line = bank * 32 + index        (bank A=0 B=1 C=2 ... I=8)"
        echo "    PC7  = 2 * 32 + 7 = 71          on the main pinctrl"
        echo "    PL10 = 10                       on the R_PIO controller"
        echo "    -> gpio_chip: <main pinctrl label>, dc_pin: 71"
        ;;
    *)
        echo "  Board family not recognised. Read the label and line count off"
        echo "  the list above and check your board's pinout for the mapping"
        echo "  between header pins and controller line offsets."
        ;;
esac
echo ""
echo "  gpio_chip accepts a label (gpio3), a path (/dev/gpiochip3) or a"
echo "  number (3). Plugin drivers do not read lymons.yaml, so for those set"
echo "  the LYMONS_GPIO_CHIP environment variable to the same value."
echo ""

# A bus with nothing on it can take a while to probe, and boards with video
# outputs expose DDC buses that are slow and uninteresting, so cap each scan.
scan_bus() {
    if command -v timeout > /dev/null 2>&1; then
        timeout 10 i2cdetect -y "$1" 2>/dev/null
        [ $? -eq 124 ] && echo "    (timed out, probably a DDC or unconnected bus)"
    else
        i2cdetect -y "$1" 2>/dev/null
    fi
}

echo "I2C scan  (confirm the panel is answering, usually 0x3C or 0x3D)"
echo "--------"
if command -v i2cdetect > /dev/null 2>&1; then
    if ls /dev/i2c-* > /dev/null 2>&1; then
        for b in /dev/i2c-*; do
            n=${b#/dev/i2c-}
            echo "  bus ${n}:"
            scan_bus "${n}" | sed 's/^/    /'
        done
    else
        echo "  no I2C buses to scan"
    fi
else
    echo "  i2cdetect not installed: $(pkg_hint i2c-tools)"
fi
echo ""

echo "Group membership  (needed for non-root access)"
echo "----------------"
GROUPS_NOW=$(id -nG 2>/dev/null)
echo "  you are in: ${GROUPS_NOW}"
for g in i2c spi gpio; do
    if getent group "$g" > /dev/null 2>&1 || grep -q "^${g}:" /etc/group 2>/dev/null; then
        case " ${GROUPS_NOW} " in
            *" ${g} "*) ;;
            *) echo "  missing '${g}': sudo usermod -aG ${g} \$USER   (log out and back in)" ;;
        esac
    fi
done
