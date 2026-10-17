METSÄVALUAATIO
==============

Asennus
-------

#### Arch linux:
```bash
git clone git@github.com:jussinokelainen/metsavaluaatio.git
cd metsavaluaatio/pkgbuild/arch
makepkg -si
```

#### macOS (homebrew):
```bash
brew tap jussinokelainen/metsavaluaatio
brew trust jussinokelainen/metsavaluaatio
brew install metsavaluaatio
```

#### Cargo

Ohjelman voi myös asentaa käyttäen cargoa, jolloin voi valita halutessaan
minkä vaan commitin josta asentaa.

```bash
git clone git@github.com:jussinokelainen/metsavaluaatio.git
cd metsavaluaatio
cargo install --path .
```

Muista lisätä `~/.cargo/bin` `$PATH` muuttujaan asentaessa cargolla esim:

```bash
export PATH="$HOME/.cargo/bin:$PATH"
```

##### Riippuvuudet:
```bash
rust
cmake
sqlite
```

Käyttö
------

Ennen ohjelman varsinaista käyttöä täytyy lisätä henkilökohtainen MML api-avain.
Tämän voi joko tehdä itse luomalla tiedosto ~/.config/metsavaluaatio/config.toml
johon lisätään kenttä `api-key = "<api-key>"`, tai komennolla
```bash
metsaval --aseta-api <api-key>
```

Api-avaimen lisäämisen jälkeen ohjelman käyttö:

```bash
metsaval 012-345-0006-0789
```

Ohjelma vaatii parametrina kiinteistötunnuksen kuten yllä olevassa esimerkissä.
Tämä tuottaa .xlsx ja .kml tiedostot siihen hakemistoon jossa se ajetaan, ja
avaa selaimen osoitteessa jossa .kml tiedostosta voi tehdä karttatason.

#### Hyödyllisiä flageja
```bash
metsaval --gpkg-kohde
metsaval --gpkg-tiedosto
```

`--gpkg-kohde` määrittää sijainnin johon GeoPackage ladataan, jos ei halua
käyttää vakio /tmp kohdetta.

`--gpkg-tiedosto` määrittää ohjelmalle tiedoston josta GeoPackage haetaan. Käyttämällä
tätä flagia voi välttää GeoPackage:n lataamisen joka kerta uudestaan, jos tietää
kiinteistön olevan samassa kunnassa kuin aikaisemmin haetun kiinteistön GeoPackage, esim:

```bash
metsaval 012-345-0006-0789 --gpkg-tiedosto /tmp/metsavaluaatio_123456/MV_Kunta.gpkg
```

HUOM! Tällä hetkellä ei ole mitään takuita mitä tapahtuu jos antaa virheellistä dataa näillä flageilla!

##### Lisätietoja muista ominaisuuksista:

```bash
metsaval --help
```

#### Kartan luominen:
Selaimessa klikkaa 'Create new map' -> Vasemalla olevassa ikkunassa 'Untitled layer' alapuolella klikkaa 'import' -> Upload -> Valitse ohjelman luoma .kml tiedosto

Klikkaa lisätyn tason yläpuolella 'Share' ja nimeä kartta -> valtuuta kaikki kenellä on linkki avaamaan kartta -> kopioi linkki alareunasta

Ohjelman poistaminen
--------------------

Package managerin avulla, tai cargolla (riippuen asennustavasta):
```bash
cargo uninstall metsaval
```
