METSÄVALUAATIO
==============

Asennus
-------

```bash
git clone git@github.com:jussinokelainen/metsavaluaatio.git
cd metsavaluaatio
make
sudo make install
```

Asennus jos käytössä macOS, jotta asennus ei
epäonnistu käyttöoikeuksien takia:
```bash
sudo make install PREFIX=/usr/local
```

Riippuvuudet:
```bash
rust
make
cmake
```

Käyttö
------

Ennen ohjelman käyttöä, luo tiedosto ~/.config/metsavaluaatio/config.toml jossa
on henkilökohtainen MML api avain.
```toml
api_key = "12345678-90ab-cdef-ghij-klmnopqrstuv"
```

Api-avaimen lisäämisen jälkeen ohjelman käyttö:

```bash
metsaval 123-456-7890-1234 Kunta
```

Ohjelma vaatii parametreina kiinteistötunnuksen ja kunnan,
kuten yllä olevassa esimerkissä.
Tuottaa .xlsx ja .kml tiedostot siihen hakemistoon jossa se ajetaan, ja
avaa selaimen osoitteessa jossa .kml tiedostosta voi tehdä karttatason.

Ohjelman poistaminen
--------------------

```bash
sudo make uninstall
```
HUOM: vaatii saman PREFIX kuin asentaessa.
