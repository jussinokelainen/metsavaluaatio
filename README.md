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

Manuaalisesti:

```bash
git clone git@github.com:jussinokelainen/metsavaluaatio.git
cd metsavaluaatio
make
sudo make install
```

Riippuvuudet:
```bash
rust
cmake
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

lisätietoja muista ominaisuuksista saa komennolla

```bash
metsaval --help
```

#### Kartan luominen:
Selaimessa klikkaa 'Create new map' -> Vasemalla olevassa ikkunassa 'Untitled layer' alapuolella klikkaa 'import' -> Upload -> Valitse ohjelman luoma .kml tiedosto

Klikkaa lisätyn tason yläpuolella 'Share' ja nimeä kartta -> valtuuta kaikki kenellä on linkki avaamaan kartta -> kopioi linkki alareunasta

Ohjelman poistaminen
--------------------

Package managerin avulla, tai manuaalisen asennuksen jälkeen:
```bash
sudo make uninstall
```
HUOM: vaatii saman PREFIX kuin asentaessa.
